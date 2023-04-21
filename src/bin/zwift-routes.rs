use anyhow::Result;
use clap::Parser;
use env_logger::Builder;
use html_parser::{Dom, Node};
use log::{debug, LevelFilter};
use zwift_data::html_query;

async fn download_webpage(url: &str) -> Result<String> {
    debug!("Downloading web page {url}...");
    Ok(reqwest::get(url).await?.text().await?)
}

#[derive(Parser, Debug)]
struct Args {
    /// Routes web page
    web_page: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Builder::from_default_env();
    builder.format_timestamp_micros().init();
    builder.filter_level(LevelFilter::Debug);

    let args = Args::parse();

    let web_page = download_webpage(&args.web_page).await?;
    let dom = Dom::parse(&web_page)?;

    let tables = html_query::select(&dom, "table").await?;
    for table in &tables {
        let rows = html_query::find(table, "tr").await?;
        let mut first_row = true;
        for row in &rows {
            let cells = html_query::find(row, if first_row { "th" } else { "td" }).await?;
            for cell in &cells {
                let mut text_found = false;
                for child in &cell.children {
                    if let Node::Text(text) = child {
                        print!("{text},");
                        text_found = true;
                        break;
                    } else if let Node::Element(element) = child {
                        if element.name == "a" {
                            for child in &element.children {
                                if let Node::Text(text) = child {
                                    print!("{text},");
                                    text_found = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if !text_found {
                    for child in &cell.children {
                        print!("{:?}", child);
                    }
                }
            }
            println!();
            first_row = false;
        }
    }

    Ok(())
}
