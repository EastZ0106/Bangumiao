export interface Subscription {
  id: string;
  title: string;
  rss_url: string;
  mikan_url: string;
  cover_url: string;
  enabled: boolean;
  auto_download: boolean;
  created_at: string;
}

export interface DownloadItem {
  id: string;
  episode_title: string;
  status: string;
  progress: number;
  file_path: string;
  subscription_title?: string;
  subscription_id?: string;
  episode_number?: number;
  gid?: string;
}

export interface PendingGroup {
  subscription_title: string;
  subscription_id: string;
  episodes: DownloadItem[];
}

export interface LibraryEpisode {
  file_path: string;
  episode_number: number | null;
  episode_title: string;
  downloaded: boolean;
  watched: boolean;
  file_name: string;
}

export interface AnimeGroup {
  title: string;
  episodes: LibraryEpisode[];
}

export interface AppSettings {
  download_dir: string;
  refresh_interval: number;
  aria2_port: number;
  max_concurrent_downloads: number;
  auto_delete_torrent: boolean;
  close_to_tray: boolean;
}

export interface SubgroupInfo {
  subgroup_id: string;
  subgroup_name: string;
  anime_title: string;
  bangumi_id: string;
}

export interface RssEpisode {
  title: string;
  torrent_url: string;
  magnet_uri: string;
  pub_date: string;
  episode_number: number | null;
}

export interface RefreshResult {
  new_episodes: number;
  started_downloads: number;
}
