//! Who leads which service, from kerktijden.nl (the same data as their website widget).

use anyhow::Result;
use chrono::NaiveDateTime;
use serde::Deserialize;

/// HHG Tholen, De Levensbron.
pub const COMMUNITY_ID: u32 = 57;

#[derive(Debug, Clone)]
pub struct Gathering {
    pub start: NaiveDateTime,
    /// e.g. "ds. A. den Hartog jr."
    pub preacher: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiGathering {
    start_time: String,
    #[serde(default)]
    persons: Vec<ApiPerson>,
}

#[derive(Deserialize)]
struct ApiPerson {
    status: Option<ApiStatus>,
    initials: Option<String>,
    insertion: Option<String>,
    lastname: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiStatus {
    status_short: Option<String>,
}

pub async fn gatherings(community: u32, weeks: u32) -> Result<Vec<Gathering>> {
    let url = format!("https://api.kerktijden.nl/api/gathering/GetGatheringsForWidget?communityId={community}&weeks={weeks}");
    let list: Vec<ApiGathering> = serde_json::from_str(&crate::extract::http_get(&url).await?)?;
    Ok(list
        .into_iter()
        .filter_map(|g| {
            let start = NaiveDateTime::parse_from_str(&g.start_time, "%Y-%m-%dT%H:%M:%S").ok()?;
            let preacher = g
                .persons
                .first()
                .map(|p| {
                    let parts = [
                        p.status.as_ref().and_then(|s| s.status_short.clone()).unwrap_or_default(),
                        p.initials.clone().unwrap_or_default(),
                        p.insertion.clone().unwrap_or_default(),
                        p.lastname.clone().unwrap_or_default(),
                    ];
                    parts.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
                })
                .unwrap_or_default();
            Some(Gathering { start, preacher })
        })
        .collect())
}
