use std::path::{PathBuf};
use tokio::io::AsyncBufReadExt;
use tokio::task;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender};
use colored::*;

async fn find_in_file(file: PathBuf, term: &str, sender: Sender<(String, String)>) {
    let file_name = file.display().to_string();
    let file = match tokio::fs::File::open(&file).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open file '{}': {}", file_name, e);
            return;
        }
    };

    let buf = tokio::io::BufReader::new(file);
    let mut lines = buf.lines();
    while let line_result = lines.next_line().await {
        let line_option = match line_result {
            Ok(line) => line,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    // Ignore invalid UTF-8 errors
                    return;
                } else {
                    eprintln!("Error reading line from file '{}': {}", file_name, e);
                    return;
                }
            }
        };

        match line_option {
            Some(line) => {
                if line.contains(term) {
                    let result = (file_name.clone(), line);
                    if sender.send(result).await.is_err() {
                        eprintln!("Failed to send result");
                    }
                }
            }
            None => return,
        }
    }
}


async fn handle_dir(dir: PathBuf, term: &'static str, sender: Sender<(String, String)>) {
    let mut tasks = Vec::new();
    let mut dirs_to_visit = vec![dir];

    while let Some(current_dir) = dirs_to_visit.pop() {
        let mut read_dir = match tokio::fs::read_dir(&current_dir).await {
            Ok(rd) => rd,
            Err(e) => {
                eprintln!("Failed to read directory '{}': {}", current_dir.display(), e);
                continue;
            }
        };

        while let Some(entry) = read_dir.next_entry().await.unwrap() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name() {
                    // Skip directories with invalid UTF-8 names
                    if let Some(name_str) = name.to_str() {
                        if !name_str.starts_with('.') {
                            dirs_to_visit.push(path);
                        }
                    }
                }
            } else if path.is_file() {
                if let Some(name) = path.file_name() {
                    // Skip files with invalid UTF-8 names
                    if name.to_str().is_some() {
                        let sender = sender.clone();
                        let task = task::spawn(async move {
                            find_in_file(path, term, sender).await;
                        });
                        tasks.push(task);
                    }
                }
            }
        }
    }

    for task in tasks {
        if let Err(e) = task.await {
            eprintln!("Task failed: {}", e);
        }
    }
}

//we need a static lifetime for the input
static mut SEARCH :String=String::new();

#[tokio::main]
async fn main() {
    let args:Vec<String> = std::env::args().collect();

    if args.len() <= 1 {
        println!("Please enter a search term.");
        return;
    }
    
    // Convert the search term to a Box<String> and leak it to get a 'static str
    // let term: &'static str = Box::leak(Box::new(args[1].clone()));
    unsafe{SEARCH=args[1].clone();}
    let term = unsafe{SEARCH.as_str()};
    // let term = &args[1];

    println!("You entered: {}", term);

    let current_dir = std::env::current_dir().expect("Failed to get current directory");

    let (sender, mut receiver) = mpsc::channel(100);

    tokio::spawn(async move {
        handle_dir(current_dir, term, sender).await;
    });

    while let Some((file_name, found_line)) = receiver.recv().await {
        println!("{}: {}", file_name, found_line.green());
    }
}
