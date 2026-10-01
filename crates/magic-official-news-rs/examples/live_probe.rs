use magic_official_news_rs::{OfficialNewsClient, OfficialSource};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let source =
        match arguments.next().as_deref() {
            Some("nbs") => OfficialSource::Nbs,
            Some("pbc") => OfficialSource::Pbc,
            Some("ndrc") => OfficialSource::Ndrc,
            Some("mof") => OfficialSource::Mof,
            Some("miit") => OfficialSource::Miit,
            Some("mofcom") => OfficialSource::Mofcom,
            Some("gacc") => OfficialSource::Gacc,
            Some("nea") => OfficialSource::Nea,
            Some("csrc") => OfficialSource::Csrc,
            _ => return Err(
                "usage: live_probe nbs|pbc|ndrc|mof|miit|mofcom|gacc|nea|csrc [--diagnostic] [--serial-load|exact-article-url]"
                    .into(),
            ),
        };
    let mut operation = arguments.next();
    let diagnostic = operation.as_deref() == Some("--diagnostic");
    if diagnostic {
        operation = arguments.next();
    }
    if arguments.next().is_some() {
        return Err("unexpected additional argument".into());
    }
    let client = OfficialNewsClient::new(source)?;
    // CSRC's aggregate includes two verified original channels. Sample both.
    let limit = if source == OfficialSource::Csrc {
        18
    } else {
        2
    };
    let latest = || {
        if diagnostic {
            client.probe_latest(limit)
        } else {
            client.latest(limit)
        }
    };
    let original = |url: &str| {
        if diagnostic {
            client.probe_article(url)
        } else {
            client.article(url)
        }
    };
    if let Some(url) = operation {
        if url == "--serial-load" {
            for _ in 0..3 {
                println!("{}", serde_json::to_string(&latest()?)?);
            }
            return Ok(());
        }
        let article = original(&url)?;
        println!("{}", serde_json::to_string(&article)?);
    } else {
        let listing = latest()?;
        println!("{}", serde_json::to_string(&listing)?);
        let mut samples: Vec<_> = listing.items.iter().take(2).collect();
        if source == OfficialSource::Csrc {
            if let Some(leadership) = listing
                .items
                .iter()
                .find(|item| item.canonical_url.as_str().contains("/csrc/c106311/"))
            {
                if !samples
                    .iter()
                    .any(|item| item.canonical_url == leadership.canonical_url)
                {
                    samples.push(leadership);
                }
            }
        }
        for reference in samples {
            let article = original(reference.canonical_url.as_str())?;
            if article.published_date != reference.published_date
                || article.title.split_whitespace().collect::<String>()
                    != reference.title.split_whitespace().collect::<String>()
            {
                return Err("listing/original publication metadata conflict".into());
            }
            println!("{}", serde_json::to_string(&article)?);
        }
    }
    Ok(())
}
