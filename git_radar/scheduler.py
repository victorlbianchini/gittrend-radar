"""
Scheduler and synchronization coordinator for GitTrend Radar.
Handles daily automatic updates, background runs, and live status reporting.
"""

import time
import logging
import threading
from datetime import datetime
from typing import Optional, Callable

from git_radar.models import SyncStatus
from git_radar.db import Database
from git_radar.scraper import GitHubScraper
from git_radar.analyzer import RepoAnalyzer

logger = logging.getLogger("git_radar.scheduler")


class SyncCoordinator:
    _instance: Optional["SyncCoordinator"] = None

    def __init__(self, db: Optional[Database] = None):
        self.db = db or Database()
        self.scraper = GitHubScraper()
        self.analyzer = RepoAnalyzer()
        self.status = SyncStatus()
        self._lock = threading.Lock()
        self._stop_event = threading.Event()
        self._thread: Optional[threading.Thread] = None
        self._update_initial_status()

    @classmethod
    def get_instance(cls, db: Optional[Database] = None) -> "SyncCoordinator":
        if cls._instance is None:
            cls._instance = SyncCoordinator(db)
        return cls._instance

    def _update_initial_status(self):
        last = self.db.get_last_sync()
        if last:
            self.status.last_sync_date = last.get("date_str")
            self.status.last_sync_time = str(last.get("sync_time"))
            self.status.total_repos_today = last.get("repos_found", 0)
            self.status.message = f"Última atualização: {self.status.last_sync_time}"
        else:
            self.status.message = "Ainda não sincronizado hoje"

    def run_sync(self, force: bool = False, since: str = "daily") -> bool:
        """
        Executes synchronization of trending repositories.
        If force=False and today is already synced, skips.
        """
        today_str = datetime.now().strftime("%Y-%m-%d")

        with self._lock:
            if self.status.is_syncing:
                logger.info("Sync already in progress. Skipping.")
                return False

            if not force and self.status.last_sync_date == today_str and self.status.total_repos_today > 0:
                logger.info(f"Today ({today_str}) already synced. Use force=True to re-sync.")
                return True

            self.status.is_syncing = True
            self.status.current_step = "Buscando repositórios em alta no GitHub..."
            self.status.message = "Buscando repositórios em alta..."

        start_time = time.time()
        logger.info(f"Starting GitHub trending sync for date {today_str}...")

        try:
            # 1. Scrape trending repos (Global + top languages)
            self.status.current_step = "Raspando tendências gerais e linguagens-chave..."
            repos = self.scraper.scrape_all_trending(since=since, date_str=today_str)
            logger.info(f"Scraped {len(repos)} repositories.")

            if not repos:
                self.status.message = "Nenhum repositório retornado pelo GitHub."
                self.status.is_syncing = False
                return False

            # 2. Save base repositories into SQLite
            self.status.current_step = f"Salvando {len(repos)} repositórios no banco de dados..."
            self.db.save_repositories(repos)

            # 3. For each repo, check if analysis exists; if not, fetch README and analyze
            total = len(repos)
            for idx, repo in enumerate(repos, start=1):
                self.status.current_step = f"Analisando ({idx}/{total}): {repo.repo_id}"
                existing_analysis = self.db.get_analysis(repo.repo_id)

                if not existing_analysis:
                    readme = self.scraper.fetch_readme(repo.owner, repo.name)
                    analysis = self.analyzer.analyze_heuristic(repo, readme)
                    self.db.save_analysis(analysis)

            duration = round(time.time() - start_time, 2)
            self.db.log_sync(
                date_str=today_str,
                repos_found=len(repos),
                new_repos=len(repos),
                status="success",
                message=f"Sincronização concluída com sucesso ({len(repos)} repositórios)",
                duration=duration
            )

            with self._lock:
                self.status.is_syncing = False
                self.status.last_sync_date = today_str
                self.status.last_sync_time = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
                self.status.total_repos_today = len(repos)
                self.status.message = f"Atualizado hoje às {datetime.now().strftime('%H:%M:%S')} ({len(repos)} repositórios)"
                self.status.current_step = "Concluído"

            logger.info(f"Sync finished in {duration}s. Total repos: {len(repos)}")
            return True

        except Exception as e:
            logger.error(f"Error during trending sync: {e}", exc_info=True)
            with self._lock:
                self.status.is_syncing = False
                self.status.message = f"Erro na sincronização: {str(e)[:100]}"
                self.status.current_step = "Erro"
            return False

    def trigger_async_sync(self, force: bool = False, since: str = "daily"):
        """Triggers sync in a detached background thread."""
        t = threading.Thread(target=self.run_sync, args=(force, since), daemon=True)
        t.start()

    def start_background_daemon(self, interval_seconds: int = 14400):
        """
        Starts a daemon that periodically checks if today has been synced.
        Default interval is 4 hours (14400s).
        """
        if self._thread and self._thread.is_alive():
            return

        def daemon_worker():
            logger.info("Daily trending background daemon started.")
            # Run initial check after 2 seconds
            time.sleep(2)
            self.run_sync(force=False)

            while not self._stop_event.is_set():
                # Sleep in increments of 10s to support quick shutdown
                for _ in range(interval_seconds // 10):
                    if self._stop_event.is_set():
                        break
                    time.sleep(10)
                
                if not self._stop_event.is_set():
                    logger.info("Daemon executing scheduled check...")
                    self.run_sync(force=False)

        self._stop_event.clear()
        self._thread = threading.Thread(target=daemon_worker, daemon=True)
        self._thread.start()

    def stop_background_daemon(self):
        self._stop_event.set()
        if self._thread:
            self._thread.join(timeout=2)
