"""
GitHub Trending Scraper and README fetcher.
Uses httpx and selectolax for high-performance server-side parsing (ZERO DOM rule).
"""

import re
import logging
from datetime import datetime
from typing import List, Dict, Any, Optional
import httpx
from selectolax.parser import HTMLParser

from git_radar.models import RepoItem

logger = logging.getLogger("git_radar.scraper")

HEADERS = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
    "Accept-Language": "en-US,en;q=0.9,pt-BR;q=0.8,pt;q=0.7",
}

CATEGORY_KEYWORDS = {
    "ia_agentes": [
        "agent", "llm", "gpt", "claude", "gemini", "deepseek", "rag", "model",
        "embedding", "transformer", "diffusion", "vision", "audio", "lora",
        "ollama", "vllm", "langchain", "llamaindex", "prompt", "reasoning",
        "fine-tune", "multimodal", "speech", "voice", "text-to-speech", "asr"
    ],
    "frontend_ui": [
        "react", "vue", "svelte", "nextjs", "tailwind", "css", "frontend",
        "ui", "component", "design system", "animation", "canvas", "webgl",
        "diagram", "editor", "theme", "html"
    ],
    "backend_api": [
        "api", "fastapi", "express", "django", "flask", "grpc", "rest",
        "database", "sql", "postgres", "sqlite", "redis", "microservice",
        "serverless", "orm", "backend", "websocket"
    ],
    "devops_cli": [
        "cli", "terminal", "docker", "kubernetes", "ci/cd", "devops",
        "monitoring", "linux", "bash", "shell", "tui", "sysadmin", "deploy",
        "proxy", "gateway", "infrastructure"
    ],
    "security": [
        "security", "reverse-engineering", "reverse engineering", "vulnerability",
        "cve", "exploit", "crypto", "auth", "pentest", "malware", "privacy",
        "firewall", "binary", "decompiler"
    ],
    "data_eng": [
        "etl", "data", "analytics", "spark", "parquet", "dataframe",
        "pandas", "polars", "pipeline", "duckdb", "warehouse", "scraper",
        "scraping", "crawler"
    ],
    "mobile": [
        "ios", "android", "swift", "flutter", "react-native", "kotlin", "mobile"
    ]
}


def parse_number(text: str) -> int:
    """Extracts integer from strings like '214,512' or '54,679 stars today'."""
    if not text:
        return 0
    # Clean commas and non-digits
    digits = re.sub(r"[^\d]", "", text)
    try:
        return int(digits) if digits else 0
    except ValueError:
        return 0


def detect_category(name: str, desc: str, lang: str) -> str:
    """Classifies repository into high-value domains based on context."""
    text = f"{name} {desc} {lang}".lower()
    for category, keywords in CATEGORY_KEYWORDS.items():
        for kw in keywords:
            if re.search(r"\b" + re.escape(kw) + r"\b", text) or kw in text:
                return category
    return "geral"


class GitHubScraper:
    def __init__(self, client: Optional[httpx.Client] = None):
        self.client = client or httpx.Client(headers=HEADERS, timeout=15.0, follow_redirects=True)

    def scrape_trending(
        self,
        language: str = "",
        since: str = "daily",
        date_str: Optional[str] = None
    ) -> List[RepoItem]:
        """
        Scrapes https://github.com/trending with language and period filter.
        since can be: 'daily', 'weekly', 'monthly'.
        """
        today = date_str or datetime.now().strftime("%Y-%m-%d")
        url = "https://github.com/trending"
        if language and language.lower() not in ("all", "todas"):
            url = f"https://github.com/trending/{language.lower()}"

        params = {}
        if since:
            params["since"] = since

        logger.info(f"Fetching GitHub Trending from: {url} with params {params}")
        try:
            resp = self.client.get(url, params=params)
            resp.raise_for_status()
        except Exception as e:
            logger.error(f"Error scraping trending from {url}: {e}")
            return []

        tree = HTMLParser(resp.text)
        rows = tree.css("article.Box-row")
        repos: List[RepoItem] = []

        for index, row in enumerate(rows, start=1):
            h2_link = row.css_first("h2 a")
            if not h2_link:
                continue

            full_path = h2_link.attributes.get("href", "").strip("/")
            parts = full_path.split("/")
            if len(parts) < 2:
                continue

            owner, name = parts[0], parts[1]
            repo_id = f"{owner}/{name}"

            # Description
            desc_node = row.css_first("p")
            description = desc_node.text().strip() if desc_node else ""

            # Language
            lang_node = row.css_first("span[itemprop=programmingLanguage]")
            detected_lang = lang_node.text().strip() if lang_node else (language.title() if language else "Unknown")

            # Stars today / this week
            stars_period_node = row.css_first("span.d-inline-block.float-sm-right")
            stars_today_text = stars_period_node.text().strip() if stars_period_node else ""
            stars_today = parse_number(stars_today_text)

            # Total stars and forks from Link--muted
            links = row.css("a.Link--muted")
            total_stars_text = links[0].text().strip() if len(links) > 0 else "0"
            forks_text = links[1].text().strip() if len(links) > 1 else "0"
            stars = parse_number(total_stars_text)
            forks = parse_number(forks_text)

            category = detect_category(repo_id, description, detected_lang)

            item = RepoItem(
                repo_id=repo_id,
                owner=owner,
                name=name,
                description=description,
                url=f"https://github.com/{repo_id}",
                language=detected_lang,
                stars=stars,
                forks=forks,
                stars_today=stars_today,
                period=since,
                category=category,
                date_collected=today,
                rank=index
            )
            repos.append(item)

        logger.info(f"Successfully scraped {len(repos)} repos from {url}")
        return repos

    def scrape_all_trending(self, since: str = "daily", date_str: Optional[str] = None) -> List[RepoItem]:
        """
        Scrapes general trending PLUS key developer languages (python, typescript, rust, go, javascript)
        and merges them without duplicates, preserving highest rank/stars.
        """
        collected: Dict[str, RepoItem] = {}

        # 1. Main trending (General)
        main_repos = self.scrape_trending(language="", since=since, date_str=date_str)
        for r in main_repos:
            collected[r.repo_id] = r

        # 2. Targeted popular languages
        target_languages = ["python", "typescript", "rust", "go", "javascript"]
        for lang in target_languages:
            try:
                lang_repos = self.scrape_trending(language=lang, since=since, date_str=date_str)
                for r in lang_repos:
                    if r.repo_id not in collected:
                        # Append with incremental rank
                        r.rank = len(collected) + 1
                        collected[r.repo_id] = r
                    else:
                        # Update with more accurate language if was Unknown
                        if collected[r.repo_id].language in ("Unknown", "N/A"):
                            collected[r.repo_id].language = r.language
            except Exception as e:
                logger.warning(f"Could not scrape {lang} trending: {e}")

        # Re-sort by stars_today descending
        final_list = list(collected.values())
        final_list.sort(key=lambda x: (x.stars_today, x.stars), reverse=True)
        for idx, item in enumerate(final_list, start=1):
            item.rank = idx

        return final_list

    def fetch_readme(self, owner: str, name: str) -> str:
        """
        Fetches repository README via raw GitHub URLs without hitting API rate limits.
        """
        candidates = [
            f"https://raw.githubusercontent.com/{owner}/{name}/HEAD/README.md",
            f"https://raw.githubusercontent.com/{owner}/{name}/main/README.md",
            f"https://raw.githubusercontent.com/{owner}/{name}/master/README.md",
            f"https://raw.githubusercontent.com/{owner}/{name}/HEAD/readme.md",
            f"https://raw.githubusercontent.com/{owner}/{name}/main/readme.md"
        ]

        for url in candidates:
            try:
                r = self.client.get(url)
                if r.status_code == 200 and len(r.text.strip()) > 50:
                    return r.text
            except Exception:
                continue

        return ""
