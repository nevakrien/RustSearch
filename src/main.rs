use std::fs::File;
use std::io::{self, BufRead};
use std::path::{PathBuf};
use walkdir::WalkDir;
use tokio::task;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender};
use colored::*;

async fn find_in_file(file: PathBuf, term: &'static str, sender: Sender<(String, String)>) {
    let file_name = file.display().to_string();
    let file = File::open(file).ok();
    if let Some(file) = file {
        let buf = io::BufReader::new(file);

        for line in buf.lines() {
            if let Ok(line) = line {
                if line.contains(term) {
                    let result = (file_name.clone(), line);
                    sender.send(result).await.unwrap();
                }
            }
        }
    }
}

async fn handle_dir(dir: PathBuf, term: &'static str, sender: Sender<(String, String)>) {
    let mut tasks = Vec::new();

    for entry in WalkDir::new(dir) {
        match entry {
            Ok(entry) => {
                let path = entry.path().to_path_buf();
                if entry.file_type().is_file() {
                    let sender = sender.clone();
                    let task = task::spawn(async move {
                        find_in_file(path, term, sender).await;
                    });
                    tasks.push(task);
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    for task in tasks {
        task.await.unwrap();
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() <= 1 {
        println!("Please enter a search term.");
        return;
    }

    // Convert the search term to a Box<String> and leak it to get a 'static str
    let term: &'static str = Box::leak(Box::new(args[1].clone()));
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
