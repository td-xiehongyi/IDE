export type SearchSortField = "name" | "path" | "extension" | "size" | "modified";
export type SearchSortDirection = "asc" | "desc";

export type SearchQuery = {
  root_path: string;
  query: string;
  extension: string | null;
  min_size: number | null;
  max_size: number | null;
  modified_after: number | null;
  modified_before: number | null;
  sort_by: SearchSortField;
  sort_direction: SearchSortDirection;
  page: number;
  page_size: number;
};

export type SearchEntry = {
  id: number;
  normalized_path: string;
  name: string;
  extension: string | null;
  kind: string;
  size: number;
  modified_ms: number | null;
};

export type SearchResult = {
  entries: SearchEntry[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
};

export type ExplorerChildrenQuery = {
  root_path: string;
  parent_path: string | null;
};

export type IndexedFileQuery = {
  root_path: string;
  file_path: string;
};

export type ExplorerNode = {
  id: number;
  normalized_path: string;
  name: string;
  extension: string | null;
  kind: "file" | "directory" | "symlink" | "junction";
  size: number;
  modified_ms: number | null;
  has_children: boolean;
};
