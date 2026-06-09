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
    pub mex_date: String,
    pub tags_packed: String,
    pub tag_types_packed: String,
    pub os_date: Option<String>,
    pub caption: Option<String>,
    pub source_path: String,
    pub status: Status,
    pub missing_on_disk: bool,
}

impl MediaItem {
    // Utility for full file name when it has a target path
    pub fn file_name(&self) -> Option<String> {
        self.path_stem
            .as_ref()
            .map(|stem| format!("{}{}", stem, self.ext))
    }

    #[allow(dead_code)]
    pub fn relative_path(&self) -> Option<PathBuf> {
        self.path_stem.as_ref().map(|stem| {
            let year = &stem[0..4]; // yyyy prefix
            PathBuf::from(year).join(format!("{}{}", stem, self.ext))
        })
    }

    pub fn year_str(&self) -> Option<&str> {
        let stem = self.path_stem.as_ref()?;
        stem.split('-').next()
    }

    pub fn month_str(&self) -> Option<&str> {
        let stem = self.path_stem.as_ref()?;
        if stem.len() >= 7 {
            Some(&stem[0..7])
        } else {
            None
        }
    }

    pub fn slug_str(&self) -> Option<&str> {
        let stem = self.path_stem.as_ref()?;
        let parts: Vec<&str> = stem.split('-').collect();
        if parts.len() >= 3 {
            let len = parts[0].len() + 1 + parts[1].len() + 1 + parts[2].len();
            Some(&stem[0..len])
        } else {
            None
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_name_includes_extension_correctly() {
        let media = MediaItem {
            id: "1".into(),
            path_stem: Some("2022-01-01-0001".into()),
            ext: ".jpg".into(), // extension inherently includes the dot
            mex_date: "2022-01-01".into(),
            tags_packed: "".into(),
            tag_types_packed: "".into(),
            os_date: None,
            caption: None,
            source_path: "/path".into(),
            status: Status::Normal,
            missing_on_disk: false,
        };
        // The regression caused a double dot: "2022-01-01-0001..jpg"
        // Ensure this method simply concatenates the stem and the ext
        assert_eq!(media.file_name(), Some("2022-01-01-0001.jpg".into()));
    }
}
