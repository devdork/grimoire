use log::{debug, error, info, log_enabled, Level};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
// TODO: move away from askama
use askama::Template;
use chrono::{DateTime, Local, TimeZone};
use fs_extra::dir::{copy, CopyOptions};
use glob::glob;
use minify::html::minify;
use pulldown_cmark::{html, Options, Parser};
use scraper::{Html, Selector};
use std::io::Write;

// TODO: move to errors handled using Snafu

#[derive(Debug, Clone)]
/// This is the representation for a post.
struct Post {
    filename: String,
    title: String,
    content: String,
    date: String,
}

#[derive(Template)]
#[template(path = "index.html.tmpl", escape = "none")]
/// This is the template used for index.html so that a post list can be embedded into it.
struct IndexTemplate {
    posts: Vec<Post>,
}

#[derive(Template)]
#[template(path = "post.html.tmpl", escape = "none")]
/// This is the template used for a post.
struct PostTemplate {
    post: Post,
}

/// This generates the file structure for the project's output.
fn generate_file_structure(cwd: &PathBuf) -> io::Result<()> {
    let path_suffixes = ["gen/", "gen/posts/", "gen/assets/"];
    let paths_to_create: Vec<PathBuf> = path_suffixes
        .iter()
        .map(|path_suffix| cwd.join(path_suffix))
        .collect();
    for path in paths_to_create.iter() {
        if !std::path::Path::new(&path).exists() {
            info!("Created path: {:?}.", path);
            fs::create_dir(path)?;
        }
    }
    Ok(())
}

/// This converts markdown into HTML so that posts can be turned into HTML.
fn md_to_html(file_content: String) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&file_content, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// This converts between the system time and date time formats so that a post time can be set.
fn system_time_to_date_time(t: SystemTime) -> DateTime<Local> {
    let (sec, nsec) = match t.duration_since(UNIX_EPOCH) {
        Ok(dur) => (dur.as_secs() as i64, dur.subsec_nanos()),
        Err(e) => {
            let dur = e.duration();
            let (sec, nsec) = (dur.as_secs() as i64, dur.subsec_nanos());
            if nsec == 0 {
                (-sec, 0)
            } else {
                (-sec - 1, 1_000_000_000 - nsec)
            }
        }
    };
    Local.timestamp(sec, nsec)
}

/// This generates an instance of Post from the information about that post.
fn generate_post(path: PathBuf, post_title_selector: &scraper::Selector) -> io::Result<Post> {
    let post_filename = String::from(path.to_str().ok_or(io::Error::new(
        io::ErrorKind::InvalidInput,
        "Filename does not create a valid string",
    ))?)
    .replace("md", "html");
    let post_content = fs::read_to_string(&path)?;
    let post_html = md_to_html(post_content);
    let fragment = Html::parse_fragment(&post_html);
    let post_title = fragment
        .select(&post_title_selector)
        .next()
        .ok_or(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Post has no title",
        ))?
        .inner_html();
    let post_date = system_time_to_date_time(fs::metadata(&path)?.modified()?)
        .format("%F %T %Z")
        .to_string();
    Ok(Post {
        filename: post_filename,
        content: post_html,
        date: post_date,
        title: post_title,
    })
}

// TODO: find a way to have it return alongside the errors which file they actually relate to, eventually
/// This generates a vector of posts from a folder full of markdown files.
fn generate_posts_list() -> std::result::Result<Vec<Post>, Vec<io::Error>> {
    let post_title_selector = Selector::parse("h2").unwrap();
    let unsanitized_posts = glob("./posts/*.md")
        .expect("Glob error")
        .map(|path| {
            generate_post(
                path.map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Path error"))?,
                &post_title_selector,
            )
        })
        .collect::<Vec<io::Result<Post>>>();
    let (posts, errors): (Vec<_>, Vec<_>) = unsanitized_posts.into_iter().partition(Result::is_ok);
    let posts: Vec<Post> = posts.into_iter().map(Result::unwrap).collect();
    let errors: Vec<_> = errors.into_iter().map(Result::unwrap_err).collect();
    if !errors.is_empty() {
        for error in &errors {
            println!("{:?}", error);
        }
        Err(errors)
    } else {
        for post in &posts {
            println!("{:?}", post);
        }
        Ok(posts)
    }
}

/// This generates each post as a HTML file from the template and the Post instance.
fn generate_posts(cwd: &PathBuf, posts: Vec<Post>) -> Result<(), io::Error> {
    for post in posts {
        let recipient_filename = cwd.join(format!("gen/{}", &post.filename));
        println!("{:?}", recipient_filename);
        let recipient_html = minify(
            &PostTemplate { post }
                .render()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Post template error"))?,
        );
        println!("{:?}", recipient_html);
        let mut file_buffer = fs::File::create(recipient_filename)?;
        file_buffer.write_all(recipient_html.as_bytes())?;
    }
    Ok(())
}
// fn generate_posts_list() -> std::result::Result<Vec<Post>, Vec<(PathBuf, &'static io::Error)>> {
//     let post_title_selector = Selector::parse("h2").unwrap();
//     let unsanitized_posts = glob("./posts/*.md").expect("Oh god oh fuck").map(|path| { generate_post(path.map_err(|_| {io::Error::new(io::ErrorKind::InvalidData, "Path error")})?, &post_title_selector) }).collect::<Vec<io::Result<Post>>>();
//     let err_iter: Vec<_> = glob("./posts/*.md").expect("Oh god oh fuck").zip(unsanitized_posts.iter()).collect();
//     // let errors: Vec<_> = err_iter.iter().filter_map(|(x,y)| {
//     //     match y {
//     //         Ok(_ok) => None,
//     //         Err(error) => Some((x,error))
//     //     }
//     // }).collect();
//     let errors: Vec<_> = err_iter.iter().filter_map(| z | {
//         match z {
//             (Ok(p), Err(error)) => Some((p.clone(),error.clone())),
//             _ => None
//         }
//     }).collect();
//     // let (posts, errors): (Vec<_>, Vec<_>) = err_iter.map(|(x, y)| y).partition(Result::is_ok);
//     // let posts: Vec<Post> = posts.into_iter().map(Result::unwrap).collect();
//     // let errors: Vec<_> = errors.into_iter().map(Result::unwrap_err).collect();
//
//     if !errors.is_empty() {
//         for error in &errors {
//             println!("{:?}", error)
//         }
//         Err(errors)
//     } else {
//         let posts: Vec<_>  = unsanitized_posts.iter().filter_map(| x | {
//             match x {
//                 Ok(okay) => Some(okay.clone()),
//                 _ => None
//             }
//         }).collect();
//         for post in &posts {
//             println!("{:?}", post)
//         }
//         Ok(posts)
//     }
// }

fn generate_index(cwd: &PathBuf, post_list: &Vec<Post>) -> Result<(), io::Error> {
    let recipient_html = minify(
        IndexTemplate {
            posts: post_list.clone(),
        }
        .render()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Index template error"))?
        .as_str(),
    );
    let recipient_filename = cwd.join("gen/index.html");
    let mut file_buffer = fs::File::create(recipient_filename)?;
    file_buffer.write_all(recipient_html.as_bytes())?;
    Ok(())
}

/// This is where the magic happens.
fn main() {
    env_logger::init();
    let mut cwd: PathBuf;
    match std::env::current_exe() {
        Ok(path) => cwd = path,
        Err(error) => panic!("Can't find the executable's directory: {:?}.", error),
    };
    cwd.pop();
    info!("Checking required file structures for generation.");
    match generate_file_structure(&cwd) {
        Ok(_ok) => (),
        Err(error) => panic!("Error in generating the file structure: {:?}.", error),
    }
    info!("Checked required file structures for generation.");
    info!("Generating posts list from posts directory.");
    let post_list: Vec<Post>;
    match generate_posts_list() {
        Ok(posts) => post_list = posts,
        Err(errors) => panic!("Errors in the post list generation: {:?}", errors),
    }
    info!("Generated posts list from posts directory.");
    info!("Copying static assets.");
    let options = CopyOptions::new();
    copy("./assets/", &cwd.join("gen"), &options);
    info!("Copied static assets.");
    info!("Generating index page from the post list.");
    match generate_index(&cwd, &post_list) {
        Ok(_ok) => (),
        Err(error) => panic!("Error in generating index file: {:?}.", error),
    }
    info!("Generated index page from the post list.");
    info!("Generating files for each post.");
    match generate_posts(&cwd, post_list) {
        Ok(_ok) => (),
        Err(error) => panic!("Error in generating post files: {:?}.", error),
    }
    info!("Generated files for each post.")
}
