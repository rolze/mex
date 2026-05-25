use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Imported,
    Normal,
    Duplicate,
    Trashed,
    Deleted,
}

impl Status {
    pub fn from_str(s: &str) -> Self {
        match s {
            "imported" => Status::Imported,
            "normal" => Status::Normal,
            "duplicate" => Status::Duplicate,
            "trashed" => Status::Trashed,
            "deleted" => Status::Deleted,
            _ => Status::Imported,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Imported => "imported",
            Status::Normal => "normal",
            Status::Duplicate => "duplicate",
            Status::Trashed => "trashed",
            Status::Deleted => "deleted",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaItem {
    pub id: String,
    pub path_stem: Option<String>,
    pub ext: String,
    pub derived_date: String,
    pub tags_packed: String,
    pub tag_types_packed: String,
    pub orig_os_date: Option<String>,
    pub caption: Option<String>,
    pub source_path: String,
    pub status: Status,
    pub missing_on_disk: bool,
}

impl MediaItem {
    // Utility for full file name when it has a target path
    pub fn file_name(&self) -> Option<String> {
        self.path_stem.as_ref().map(|stem| format!("{}{}", stem, self.ext))
    }

    pub fn relative_path(&self) -> Option<PathBuf> {
        self.path_stem.as_ref().map(|stem| {
            let year = &stem[0..4]; // yyyy prefix
            PathBuf::from(year).join(format!("{}{}", stem, self.ext))
        })
    }

    pub fn group_key(&self) -> Option<String> {
        // According to REGEXP.md (implied by UC-04):
        // Files with a slug: yyyy-MM-<slug>-... -> group by yyyy-MM-<slug>
        // Files without a slug: yyyy-MM-DD-... -> group by yyyy-MM-DD
        // stem: "2022-04-18-0001" or "2022-04-vacation-0001" or "2022-04-18-caption"
        let stem = self.path_stem.as_ref()?;
        let parts: Vec<&str> = stem.split('-').collect();
        if parts.len() >= 3 {
            // Check if the 3rd part is a day (2 digits) or a slug
            let third = parts[2];
            if third.len() == 2 && third.chars().all(|c| c.is_ascii_digit()) {
                // Day format: yyyy-MM-DD
                Some(format!("{}-{}-{}", parts[0], parts[1], parts[2]))
            } else {
                // Slug format: yyyy-MM-<slug>
                Some(format!("{}-{}-{}", parts[0], parts[1], parts[2]))
            }
        } else {
            None
        }
    }
}
