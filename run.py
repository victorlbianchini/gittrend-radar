"""
GitTrend Radar - Application Launcher
Starts the FastAPI server and launches the dashboard in Brave Browser.
"""

import os
import sys
import time
import shutil
import logging
import webbrowser
import subprocess
import threading
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(BASE_DIR))

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s"
)
logger = logging.getLogger("GitTrendRadar")

PORT = 8765
HOST = "127.0.0.1"
URL = f"http://{HOST}:{PORT}"

def find_brave_path() -> str:
    """Finds Brave Browser executable on Windows."""
    candidates = [
        os.path.expandvars(r"%LOCALAPPDATA%\BraveSoftware\Brave-Browser\Application\brave.exe"),
        os.path.expandvars(r"%PROGRAMFILES%\BraveSoftware\Brave-Browser\Application\brave.exe"),
        os.path.expandvars(r"%PROGRAMFILES(X86)%\BraveSoftware\Brave-Browser\Application\brave.exe"),
        r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
        shutil.which("brave") or ""
    ]
    for p in candidates:
        if p and os.path.exists(p):
            return p
    return ""

def open_browser():
    """Opens the application in Brave Browser (or default browser fallback)."""
    time.sleep(1.2)
    brave = find_brave_path()
    if brave:
        logger.info(f"Abrindo GitTrend Radar no Brave Browser: {brave}")
        try:
            subprocess.Popen([brave, URL])
            return
        except Exception as e:
            logger.warning(f"Falha ao iniciar Brave diretamente ({e}), usando fallback.")
    
    # Fallback
    logger.info(f"Abrindo GitTrend Radar no navegador padrao: {URL}")
    webbrowser.open(URL)

def print_banner():
    banner = f"""
========================================================================
   ____ _ _  _____  _____ _   _ ____    ____      _    ____    _    ____  
  / ___(_) ||_   _|| ____| \\ | |  _ \\  |  _ \\    / \\  |  _ \\  / \\  |  _ \\ 
 | |  _| | __|| |  |  _| |  \\| | | | | | |_) |  / _ \\ | | | |/ _ \\ | |_) |
 | |_| | | |_ | |  | |___| |\\  | |_| | |  _ <  / ___ \\| |_| / ___ \\|  _ < 
  \\____|_|\\__||_|  |_____|_| \\_|____/  |_| \\_\\/_/   \\_\\____/_/   \\_\\_| \\_\\

  Radar Diario Inteligente de Tendencias do GitHub
  Servidor Ativo em: {URL}
========================================================================
"""
    print(banner)

def main():
    print_banner()

    # Pre-sync verification
    from git_radar.db import Database
    from git_radar.scheduler import SyncCoordinator
    db = Database()
    coordinator = SyncCoordinator(db)
    
    # Trigger browser opening in parallel thread
    threading.Thread(target=open_browser, daemon=True).start()

    # Start FastAPI server
    import uvicorn
    uvicorn.run("git_radar.server:app", host=HOST, port=PORT, log_level="info", reload=False)

if __name__ == "__main__":
    main()
