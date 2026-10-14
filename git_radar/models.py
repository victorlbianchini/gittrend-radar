"""
Models for GitTrend Radar.
Data structures for repositories, analysis, and daily digests.
"""

from typing import List, Optional, Dict, Any
from pydantic import BaseModel, Field
from datetime import datetime


class RepoItem(BaseModel):
    repo_id: str  # "owner/name"
    owner: str
    name: str
    description: str = ""
    url: str
    language: str = "Unknown"
    stars: int = 0
    forks: int = 0
    stars_today: int = 0
    period: str = "daily"  # "daily", "weekly", "monthly"
    category: str = "geral"
    date_collected: str  # "YYYY-MM-DD"
    rank: int = 1
    is_favorite: bool = False
    has_analysis: bool = False
    simple_summary: Optional[str] = None
    why_trending: Optional[str] = None
    when_to_select: Optional[List[str]] = None


class RepoAnalysis(BaseModel):
    repo_id: str
    simple_summary: str
    why_trending: str
    code_example: str
    when_to_select: List[str]
    when_to_avoid: List[str]
    quick_verdict: str
    key_features: List[str]
    tech_stack: List[str]
    target_audience: str
    install_cmd: str = ""
    analyzed_at: str = Field(default_factory=lambda: datetime.now().isoformat())
    analysis_source: str = "heuristic"  # "heuristic", "ollama", "gemini", "openai"


class SyncStatus(BaseModel):
    is_syncing: bool = False
    last_sync_time: Optional[str] = None
    last_sync_date: Optional[str] = None
    total_repos_today: int = 0
    message: str = "Pronto"
    current_step: str = ""


class DailyDigest(BaseModel):
    date: str
    total_repos: int
    top_gainer: Optional[Dict[str, Any]] = None
    dominant_language: str = "N/A"
    dominant_category: str = "N/A"
    headline: str = ""
    highlights: List[str] = []
