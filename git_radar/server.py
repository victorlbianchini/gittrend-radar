"""
FastAPI application for GitTrend Radar.
Provides REST endpoints and serves the modern reactive dashboard.
"""

import os
import logging
from pathlib import Path
from typing import Optional, List, Dict, Any

from fastapi import FastAPI, Query, HTTPException, BackgroundTasks
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse, PlainTextResponse

from git_radar.db import Database
from git_radar.models import SyncStatus, DailyDigest
from git_radar.scheduler import SyncCoordinator
from git_radar.scraper import GitHubScraper
from git_radar.analyzer import RepoAnalyzer

logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(name)s: %(message)s")
logger = logging.getLogger("git_radar.server")

BASE_DIR = Path(__file__).resolve().parent.parent
STATIC_DIR = BASE_DIR / "static"

app = FastAPI(
    title="GitTrend Radar API",
    description="Radar diário inteligente de tendências do GitHub com análises explicativas e casos de uso",
    version="1.0.0"
)

# CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Global services
db = Database()
coordinator = SyncCoordinator.get_instance(db)
scraper = GitHubScraper()
analyzer = RepoAnalyzer()


@app.on_event("startup")
async def startup_event():
    """Start background sync daemon on application launch."""
    coordinator.start_background_daemon()


@app.on_event("shutdown")
async def shutdown_event():
    coordinator.stop_background_daemon()


# ---------------- API Endpoints ----------------

@app.get("/api/status", response_model=SyncStatus)
def get_status():
    """Returns current sync status, last update timestamp and repo count."""
    return coordinator.status


@app.post("/api/sync")
def trigger_sync(force: bool = False, since: str = "daily", background_tasks: BackgroundTasks = None):
    """Triggers an immediate update / sync."""
    if coordinator.status.is_syncing:
        return {"status": "already_syncing", "message": "Sincronização já em andamento."}

    coordinator.trigger_async_sync(force=force, since=since)
    return {"status": "started", "message": "Sincronização iniciada em segundo plano."}


@app.get("/api/trends")
def get_trends(
    date: Optional[str] = None,
    period: str = "daily",
    language: Optional[str] = None,
    category: Optional[str] = None,
    search: Optional[str] = None,
    sort_by: str = "stars_today",
    limit: int = 100,
    offset: int = 0
):
    """
    Returns trending repositories matching filters with their simple explanations and recommendations.
    """
    items = db.get_trends(
        date_str=date,
        period=period,
        language=language,
        category=category,
        search=search,
        sort_by=sort_by,
        limit=limit,
        offset=offset
    )
    return {"date": date, "count": len(items), "repositories": items}


@app.get("/api/repo/{owner}/{name}")
def get_repo_detail(owner: str, name: str):
    """Returns full details, simple explanation, practical code example and decision guide."""
    repo_id = f"{owner}/{name}"
    data = db.get_repo_detail(repo_id)
    if not data:
        raise HTTPException(status_code=404, detail="Repositório não encontrado")

    # If analysis is missing, generate heuristic analysis on the fly
    if not data.get("analysis"):
        readme = scraper.fetch_readme(owner, name)
        from git_radar.models import RepoItem
        item = RepoItem(
            repo_id=repo_id,
            owner=owner,
            name=name,
            description=data.get("description", ""),
            url=data.get("url", f"https://github.com/{repo_id}"),
            language=data.get("language", "Unknown"),
            stars=data.get("stars", 0),
            forks=data.get("forks", 0),
            stars_today=data.get("stars_today", 0),
            period=data.get("period", "daily"),
            category=data.get("category", "geral"),
            date_collected=data.get("date_collected", "")
        )
        analysis = analyzer.analyze_heuristic(item, readme)
        db.save_analysis(analysis)
        data["analysis"] = analysis.dict()

    return data


@app.post("/api/favorites/{owner}/{name}")
def toggle_favorite(owner: str, name: str, notes: str = "", tags: str = ""):
    """Toggles favorite status for a repository."""
    repo_id = f"{owner}/{name}"
    is_fav = db.toggle_favorite(repo_id, notes=notes, tags=tags)
    return {"repo_id": repo_id, "is_favorite": is_fav}


@app.get("/api/favorites")
def get_favorites():
    """Lists all user saved/bookmarked repositories."""
    favs = db.get_favorites()
    return {"count": len(favs), "favorites": favs}


@app.post("/api/analyze/{owner}/{name}")
def reanalyze_repo(owner: str, name: str, mode: str = "heuristic", model: str = "qwen3:8b"):
    """
    Re-analyzes a repository.
    Mode can be 'heuristic' (instant) or 'ollama' (deep LLM analysis).
    """
    repo_id = f"{owner}/{name}"
    data = db.get_repo_detail(repo_id)
    if not data:
        raise HTTPException(status_code=404, detail="Repositório não encontrado")

    from git_radar.models import RepoItem
    item = RepoItem(
        repo_id=repo_id,
        owner=owner,
        name=name,
        description=data.get("description", ""),
        url=data.get("url", f"https://github.com/{repo_id}"),
        language=data.get("language", "Unknown"),
        stars=data.get("stars", 0),
        forks=data.get("forks", 0),
        stars_today=data.get("stars_today", 0),
        period=data.get("period", "daily"),
        category=data.get("category", "geral"),
        date_collected=data.get("date_collected", "")
    )

    readme = scraper.fetch_readme(owner, name)

    if mode == "ollama":
        analysis = analyzer.analyze_with_ollama(item, readme, model=model)
        if not analysis:
            # Fallback to heuristic
            analysis = analyzer.analyze_heuristic(item, readme)
    else:
        analysis = analyzer.analyze_heuristic(item, readme)

    db.save_analysis(analysis)
    return {"repo_id": repo_id, "analysis": analysis.dict()}


@app.get("/api/languages")
def get_languages(date: Optional[str] = None):
    """Lists all programming languages present in current trends."""
    langs = db.get_available_languages(date_str=date)
    return {"languages": langs}


@app.get("/api/digest", response_model=DailyDigest)
def get_digest(date: Optional[str] = None):
    """Returns today's high-level radar digest."""
    return db.get_daily_digest(date_str=date)


@app.get("/api/export")
def export_markdown(date: Optional[str] = None):
    """Exports daily trending report as clean Markdown."""
    digest = db.get_daily_digest(date_str=date)
    items = db.get_trends(date_str=digest.date, limit=50)

    lines = [
        f"# GitTrend Radar Report - {digest.date}",
        f"\n**Total de repositórios monitorados hoje:** {digest.total_repos}",
        f"**Categoria em destaque:** {digest.dominant_category.replace('_', ' ').title()}",
        f"**Linguagem predominante:** {digest.dominant_language}",
        f"\n> {digest.headline}\n",
        "## Top Repositórios do Dia\n"
    ]

    for idx, r in enumerate(items, start=1):
        lines.append(f"### {idx}. [{r['repo_id']}]({r['url']})")
        lines.append(f"- **Linguagem:** {r['language']} | **Estrelas Hoje:** +{r['stars_today']:,} | **Total:** {r['stars']:,} estrelas")
        lines.append(f"- **Categoria:** {r['category'].replace('_', ' ').title()}")
        lines.append(f"- **Resumo simples:** {r.get('simple_summary') or r.get('description')}")
        if r.get("when_to_select"):
            lines.append("- **Quando selecionar:**")
            for crit in r["when_to_select"][:3]:
                lines.append(f"  - {crit}")
        lines.append("")

    content = "\n".join(lines)
    return PlainTextResponse(content, media_type="text/markdown")


# ---------------- Static Web Dashboard ----------------

if STATIC_DIR.exists():
    app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")

@app.get("/")
def serve_index():
    index_file = STATIC_DIR / "index.html"
    if index_file.exists():
        return FileResponse(str(index_file))
    return {"message": "GitTrend Radar backend online. Frontend em carregamento."}
