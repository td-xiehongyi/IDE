use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub root_path: String,
    #[serde(default)]
    pub query: String,
    pub extension: Option<String>,
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
    pub modified_after: Option<i64>,
    pub modified_before: Option<i64>,
    pub sort_by: SearchSortField,
    pub sort_direction: SearchSortDirection,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSortField {
    Name,
    Path,
    Extension,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchEntry {
    pub id: i64,
    pub normalized_path: String,
    pub name: String,
    pub extension: Option<String>,
    pub kind: String,
    pub size: i64,
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub entries: Vec<SearchEntry>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorerChildrenQuery {
    pub root_path: String,
    pub parent_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFileQuery {
    pub root_path: String,
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplorerNode {
    pub id: i64,
    pub normalized_path: String,
    pub name: String,
    pub extension: Option<String>,
    pub kind: String,
    pub size: i64,
    pub modified_ms: Option<i64>,
    pub has_children: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_has_explicit_sort_and_paging_fields() {
        let query = SearchQuery {
            root_path: "C:/资料".into(),
            query: "报告".into(),
            extension: Some("pdf".into()),
            min_size: None,
            max_size: Some(10_000),
            modified_after: None,
            modified_before: None,
            sort_by: SearchSortField::Modified,
            sort_direction: SearchSortDirection::Desc,
            page: 1,
            page_size: 50,
        };
        assert_eq!(query.page_size, 50);
        assert_eq!(query.sort_by, SearchSortField::Modified);
    }
}
