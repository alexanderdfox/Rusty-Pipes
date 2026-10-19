use clap::Parser;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

/// XPipe: A topological cross-pipe utility for multi-stream data routing and encryption.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// West input source (file path, or leave empty for stdin)
    #[arg(long)]
    west: Option<PathBuf>,

    /// North input source (file path)
    #[arg(long)]
    north: Option<PathBuf>,

    /// East output destination (file path, or leave empty for stdout)
    #[arg(long)]
    east: Option<PathBuf>,

    /// South output destination (file path)
    #[arg(long)]
    south: Option<PathBuf>,

    /// Buffer size for bounded channels (backpressure control)
    #[arg(long, default_value_t = 1000)]
    buffer_size: usize,
}

#[derive(Debug)]
enum StreamMessage {
    WestData(Vec<u8>),
    NorthData(Vec<u8>),
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let args = Args::parse();

    let (tx, rx): (SyncSender<StreamMessage>, Receiver<StreamMessage>) = 
        mpsc::sync_channel(args.buffer_size);

    // --- 1. Initialize West Input (Binary Stream) ---
    let tx_west = tx.clone();
    let west_reader: Box<dyn Read + Send> = match args.west {
        Some(path) => Box::new(File::open(path)?),
        None => Box::new(io::stdin()),
    };

    let west_handle = thread::spawn(move || {
        let mut reader = west_reader;
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx_west.send(StreamMessage::WestData(buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // --- 2. Initialize North Input (Binary Stream) ---
    let tx_north = tx.clone();
    let north_handle = if let Some(path) = args.north {
        let north_file = File::open(path)?;
        
        Some(thread::spawn(move || {
            let mut reader = north_file;
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx_north.send(StreamMessage::NorthData(buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }))
    } else {
        None
    };

    drop(tx);

    // --- 3. Initialize Outputs ---
    let mut east_writer: Box<dyn Write + Send> = match args.east {
        Some(path) => Box::new(File::create(path)?),
        None => Box::new(io::stdout()),
    };

    let mut south_writer: Box<dyn Write + Send> = match args.south {
        Some(path) => Box::new(File::create(path)?),
        None => Box::new(io::stderr()),
    };

    // --- 4. Central Processing Node (Buffered Queue XOR Matcher) ---
    let mut west_queue: VecDeque<Vec<u8>> = VecDeque::new();
    let mut north_queue: VecDeque<Vec<u8>> = VecDeque::new();

    while let Ok(msg) = rx.recv() {
        match msg {
            StreamMessage::WestData(data) => west_queue.push_back(data),
            StreamMessage::NorthData(data) => north_queue.push_back(data),
        }

        // Process matching blocks from both queues
        while !west_queue.is_empty() && !north_queue.is_empty() {
            let west_data = west_queue.pop_front().unwrap();
            let north_data = north_queue.pop_front().unwrap();

            let min_len = std::cmp::min(west_data.len(), north_data.len());
            let encrypted: Vec<u8> = west_data[..min_len]
                .iter()
                .zip(north_data[..min_len].iter())
                .map(|(w, n)| w ^ n)
                .collect();

            east_writer.write_all(&encrypted)?;
            east_writer.flush()?;

            // If chunks were uneven in size, push the remainder back to the queue
            if west_data.len() > min_len {
                west_queue.push_front(west_data[min_len..].to_vec());
            }
            if north_data.len() > min_len {
                north_queue.push_front(north_data[min_len..].to_vec());
            }
        }
    }

    // Flush any leftover unmatched data to the South destination
    for data in west_queue {
        south_writer.write_all(&data)?;
    }
    for data in north_queue {
        south_writer.write_all(&data)?;
    }
    south_writer.flush()?;

    // --- 5. Clean Join ---
    let _ = west_handle.join();
    if let Some(handle) = north_handle {
        let _ = handle.join();
    }

    Ok(())
}
