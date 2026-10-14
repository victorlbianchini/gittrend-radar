"""
Standalone updater script for GitTrend Radar.
Can be executed manually via CLI or automatically via Windows Task Scheduler.
"""

import sys
import logging
from git_radar.db import Database
from git_radar.scheduler import SyncCoordinator

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")
logger = logging.getLogger("update_now")

def main():
    force = "--force" in sys.argv or "-f" in sys.argv
    since = "daily"
    for arg in sys.argv:
        if arg.startswith("--since="):
            since = arg.split("=")[1]

    logger.info(f"Iniciando atualização de tendências do GitHub (período: {since}, force: {force})...")
    db = Database()
    coordinator = SyncCoordinator(db)
    success = coordinator.run_sync(force=force, since=since)
    
    if success:
        logger.info(f"Sucesso! {coordinator.status.total_repos_today} repositórios atualizados para hoje.")
        sys.exit(0)
    else:
        logger.warning(f"Sincronização finalizada com status: {coordinator.status.message}")
        sys.exit(1)

if __name__ == "__main__":
    main()
