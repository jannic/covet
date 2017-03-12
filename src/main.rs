#[macro_use]
extern crate clap;
extern crate hyper;
extern crate iron;
extern crate router;
extern crate time;
extern crate urlencoded;
extern crate xml;
extern crate xmltree;

mod cli;
mod scanner;
mod message;
mod web;

use std::thread;
use std::time::Duration;

use message::scan_job::{ScanJob, InputSource, Format, ColorSpace};
use message::scan_status::AdfState;
use scanner::{Scanner, ScannerError};

fn main() {
    let matches = cli::build_cli().get_matches();
    if let Some(matches) = matches.subcommand_matches("status") {
        let host = matches.value_of("SCANNER").unwrap();
        status(host);
    } else if let Some(matches) = matches.subcommand_matches("scan") {
        let host = matches.value_of("SCANNER").unwrap();
        let format = value_t!(matches.value_of("FORMAT"), cli::Format).unwrap();
        let color = value_t!(matches.value_of("COLORSPACE"), cli::ColorSpace).unwrap();
        let source = value_t!(matches.value_of("SOURCE"), cli::Source).unwrap();
        let resolution = value_t!(matches.value_of("RESOLUTION"), u32).unwrap();
        scan(host, format.to_internal(), color.to_internal(), source, resolution)
            .unwrap_or_else(|e| println!("Error: {}", e));
    } else if let Some(matches) = matches.subcommand_matches("web") {
        let host = matches.value_of("SCANNER").unwrap();
        let port = value_t!(matches, "PORT", u16).unwrap();
        web::run_server(host, port);
    }
}

fn status(host: &str) {
    let scanner = Scanner::new(host);
    print_scan_status(&scanner).unwrap_or_else(|e| println!("Error: {}", e));
}

fn print_scan_status(scanner: &Scanner) -> Result<(), ScannerError> {
    println!("Status of scanner {}", scanner.host());
    let status = scanner.get_scan_status()?;
    println!("Scanner: {:?}, Adf: {:?}", status.scanner_state(), status.adf_state());
    Ok(())
}

impl cli::Format {
    fn to_internal(&self) -> Format {
        match self {
            &cli::Format::pdf => Format::Pdf,
            &cli::Format::jpeg => Format::Jpeg
        }
    }
}

impl cli::ColorSpace {
    fn to_internal(&self) -> ColorSpace {
        match self {
            &cli::ColorSpace::gray => ColorSpace::Gray,
            &cli::ColorSpace::color => ColorSpace::Color
        }
    }
}

fn choose_source(source: cli::Source, adf_state: AdfState) -> Result<InputSource, ScannerError> {
    let input_source = match source {
        cli::Source::auto => {
            if adf_state == AdfState::Loaded {
                InputSource::Adf
            } else {
                InputSource::Platen
            }
        },
        cli::Source::adf => {
            if adf_state == AdfState::Loaded {
                InputSource::Adf
            } else {
                return Err(ScannerError::AdfEmpty);
            }
        },
        cli::Source::glass => InputSource::Platen
    };
    Ok(input_source)
}

fn scan(host: &str, format: Format, color: ColorSpace, source: cli::Source, resolution:u32)
        -> Result<(), ScannerError> {
    let scanner = Scanner::new(host);
    let status = scanner.get_scan_status()?;
    if !status.is_idle() {
        return Err(ScannerError::Busy);
    }
    let input_source = choose_source(source, status.adf_state())?;
    let mut job = scanner.start_job(ScanJob::new(input_source, resolution, format, color))?;
    println!("Job: {:?}", job);
    loop {
        let ready = job.retrieve_status()?;
        if ready {
            println!("Job: {:?}", job);
            let output_file = scanner::output_file_name(&format, &time::now());
            job.download_to_file(&output_file)?;
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}
