"""
Database module for GitTrend Radar.
SQLite database manager for repositories, daily trends, analyses, favorites and sync logs.
"""

import sqlite3
import json
import logging
from pathlib import Path
from datetime import datetime
from typing import List, Optional, Dict, Any

from git_radar.models import RepoItem, RepoAnalysis, DailyDigest

logger = logging.getLogger("git_radar.db")

DB_PATH = Path(__file__).resolve().parent.parent / "git_trending.db"


class Database:
    def __init__(self, db_path: Path = DB_PATH):
        self.db_path = db_path
        self.init_db()

    def get_connection(self) -> sqlite3.Connection:
        conn = sqlite3.connect(str(self.db_path))
        conn.row_factory = sqlite3.Row
        return conn

    def init_db(self):
        """Creates tables and indexes if they do not exist."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            
            # Repositories table
            cursor.execute("""
            CREATE TABLE IF NOT EXISTS repositories (
                repo_id TEXT,
                owner TEXT,
                name TEXT,
                description TEXT,
                url TEXT,
                language TEXT,
                stars INTEGER DEFAULT 0,
                forks INTEGER DEFAULT 0,
                stars_today INTEGER DEFAULT 0,
                period TEXT DEFAULT 'daily',
                category TEXT DEFAULT 'geral',
                date_collected TEXT,
                rank INTEGER DEFAULT 1,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (repo_id, date_collected, period)
            )
            """)

            # Detailed Analysis table
            cursor.execute("""
            CREATE TABLE IF NOT EXISTS repo_analysis (
                repo_id TEXT PRIMARY KEY,
                simple_summary TEXT,
                why_trending TEXT,
                code_example TEXT,
                when_to_select TEXT,
                when_to_avoid TEXT,
                quick_verdict TEXT,
                key_features TEXT,
                tech_stack TEXT,
                target_audience TEXT,
                install_cmd TEXT,
                analyzed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                analysis_source TEXT DEFAULT 'heuristic'
            )
            """)

            # User Favorites
            cursor.execute("""
            CREATE TABLE IF NOT EXISTS favorites (
                repo_id TEXT PRIMARY KEY,
                notes TEXT DEFAULT '',
                tags TEXT DEFAULT '',
                saved_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            """)

            # Sync history
            cursor.execute("""
            CREATE TABLE IF NOT EXISTS sync_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                sync_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                date_str TEXT,
                repos_found INTEGER,
                new_repos INTEGER,
                status TEXT,
                message TEXT,
                duration_sec REAL
            )
            """)

            # Indexes for ultra-fast queries
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_repos_date ON repositories (date_collected)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_repos_period ON repositories (period)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_repos_lang ON repositories (language)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_repos_category ON repositories (category)")
            cursor.execute("CREATE INDEX IF NOT EXISTS idx_repos_stars_today ON repositories (stars_today DESC)")
            
            conn.commit()
            logger.info(f"Database initialized at {self.db_path}")

    def save_repositories(self, repos: List[RepoItem]) -> int:
        """Upserts repositories into database."""
        if not repos:
            return 0
        inserted = 0
        with self.get_connection() as conn:
            cursor = conn.cursor()
            for r in repos:
                cursor.execute("""
                INSERT INTO repositories (
                    repo_id, owner, name, description, url, language,
                    stars, forks, stars_today, period, category, date_collected, rank
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(repo_id, date_collected, period) DO UPDATE SET
                    description = excluded.description,
                    stars = excluded.stars,
                    forks = excluded.forks,
                    stars_today = excluded.stars_today,
                    language = excluded.language,
                    category = excluded.category,
                    rank = excluded.rank
                """, (
                    r.repo_id, r.owner, r.name, r.description, r.url, r.language,
                    r.stars, r.forks, r.stars_today, r.period, r.category, r.date_collected, r.rank
                ))
                inserted += 1
            conn.commit()
        return inserted

    def save_analysis(self, analysis: RepoAnalysis):
        """Saves or updates detailed analysis for a repository."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
            INSERT INTO repo_analysis (
                repo_id, simple_summary, why_trending, code_example,
                when_to_select, when_to_avoid, quick_verdict, key_features,
                tech_stack, target_audience, install_cmd, analyzed_at, analysis_source
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(repo_id) DO UPDATE SET
                simple_summary = excluded.simple_summary,
                why_trending = excluded.why_trending,
                code_example = excluded.code_example,
                when_to_select = excluded.when_to_select,
                when_to_avoid = excluded.when_to_avoid,
                quick_verdict = excluded.quick_verdict,
                key_features = excluded.key_features,
                tech_stack = excluded.tech_stack,
                target_audience = excluded.target_audience,
                install_cmd = excluded.install_cmd,
                analyzed_at = excluded.analyzed_at,
                analysis_source = excluded.analysis_source
            """, (
                analysis.repo_id,
                analysis.simple_summary,
                analysis.why_trending,
                analysis.code_example,
                json.dumps(analysis.when_to_select, ensure_ascii=False),
                json.dumps(analysis.when_to_avoid, ensure_ascii=False),
                analysis.quick_verdict,
                json.dumps(analysis.key_features, ensure_ascii=False),
                json.dumps(analysis.tech_stack, ensure_ascii=False),
                analysis.target_audience,
                analysis.install_cmd,
                analysis.analyzed_at,
                analysis.analysis_source
            ))
            conn.commit()

    def get_analysis(self, repo_id: str) -> Optional[RepoAnalysis]:
        """Retrieves analysis for a repository if present."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM repo_analysis WHERE repo_id = ?", (repo_id,))
            row = cursor.fetchone()
            if not row:
                return None
            return RepoAnalysis(
                repo_id=row["repo_id"],
                simple_summary=row["simple_summary"] or "",
                why_trending=row["why_trending"] or "",
                code_example=row["code_example"] or "",
                when_to_select=json.loads(row["when_to_select"] or "[]"),
                when_to_avoid=json.loads(row["when_to_avoid"] or "[]"),
                quick_verdict=row["quick_verdict"] or "",
                key_features=json.loads(row["key_features"] or "[]"),
                tech_stack=json.loads(row["tech_stack"] or "[]"),
                target_audience=row["target_audience"] or "",
                install_cmd=row["install_cmd"] or "",
                analyzed_at=str(row["analyzed_at"]),
                analysis_source=row["analysis_source"] or "heuristic"
            )

    def get_trends(
        self,
        date_str: Optional[str] = None,
        period: str = "daily",
        language: Optional[str] = None,
        category: Optional[str] = None,
        search: Optional[str] = None,
        sort_by: str = "stars_today",
        limit: int = 100,
        offset: int = 0
    ) -> List[Dict[str, Any]]:
        """Queries repositories matching filters, joining with analysis and favorite status."""
        with self.get_connection() as conn:
            cursor = conn.cursor()

            # If date_str not specified, pick the most recent date available in database
            if not date_str:
                cursor.execute("SELECT MAX(date_collected) as max_date FROM repositories WHERE period = ?", (period,))
                res = cursor.fetchone()
                date_str = res["max_date"] if res and res["max_date"] else datetime.now().strftime("%Y-%m-%d")

            conditions = ["r.date_collected = ?", "r.period = ?"]
            params: List[Any] = [date_str, period]

            if language and language.lower() != "all" and language.lower() != "todas":
                conditions.append("LOWER(r.language) = LOWER(?)")
                params.append(language)

            if category and category.lower() != "all" and category.lower() != "todas":
                conditions.append("LOWER(r.category) = LOWER(?)")
                params.append(category)

            if search and search.strip():
                term = f"%{search.strip().lower()}%"
                conditions.append("""(
                    LOWER(r.name) LIKE ? OR
                    LOWER(r.owner) LIKE ? OR
                    LOWER(r.description) LIKE ? OR
                    LOWER(COALESCE(a.simple_summary, '')) LIKE ? OR
                    LOWER(COALESCE(a.key_features, '')) LIKE ?
                )""")
                params.extend([term, term, term, term, term])

            where_clause = " AND ".join(conditions)

            order_clause = "r.stars_today DESC, r.stars DESC"
            if sort_by == "stars":
                order_clause = "r.stars DESC"
            elif sort_by == "forks":
                order_clause = "r.forks DESC"
            elif sort_by == "rank":
                order_clause = "r.rank ASC"

            query = f"""
            SELECT
                r.*,
                CASE WHEN f.repo_id IS NOT NULL THEN 1 ELSE 0 END as is_favorite,
                CASE WHEN a.repo_id IS NOT NULL THEN 1 ELSE 0 END as has_analysis,
                a.simple_summary,
                a.why_trending,
                a.when_to_select,
                a.quick_verdict,
                a.install_cmd
            FROM repositories r
            LEFT JOIN favorites f ON r.repo_id = f.repo_id
            LEFT JOIN repo_analysis a ON r.repo_id = a.repo_id
            WHERE {where_clause}
            ORDER BY {order_clause}
            LIMIT ? OFFSET ?
            """
            params.extend([limit, offset])

            cursor.execute(query, params)
            rows = cursor.fetchall()
            results = []
            for row in rows:
                item = dict(row)
                item["is_favorite"] = bool(item["is_favorite"])
                item["has_analysis"] = bool(item["has_analysis"])
                if item["when_to_select"]:
                    try:
                        item["when_to_select"] = json.loads(item["when_to_select"])
                    except Exception:
                        item["when_to_select"] = []
                else:
                    item["when_to_select"] = []
                results.append(item)
            return results

    def get_repo_detail(self, repo_id: str) -> Optional[Dict[str, Any]]:
        """Gets full repository data, analysis, and favorite status."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
            SELECT
                r.*,
                CASE WHEN f.repo_id IS NOT NULL THEN 1 ELSE 0 END as is_favorite,
                f.notes as favorite_notes
            FROM repositories r
            LEFT JOIN favorites f ON r.repo_id = f.repo_id
            WHERE r.repo_id = ?
            ORDER BY r.date_collected DESC
            LIMIT 1
            """, (repo_id,))
            repo_row = cursor.fetchone()
            if not repo_row:
                return None
            
            repo_data = dict(repo_row)
            repo_data["is_favorite"] = bool(repo_data["is_favorite"])

            # Analysis
            cursor.execute("SELECT * FROM repo_analysis WHERE repo_id = ?", (repo_id,))
            a_row = cursor.fetchone()
            if a_row:
                analysis_dict = dict(a_row)
                for json_col in ["when_to_select", "when_to_avoid", "key_features", "tech_stack"]:
                    if analysis_dict.get(json_col):
                        try:
                            analysis_dict[json_col] = json.loads(analysis_dict[json_col])
                        except Exception:
                            analysis_dict[json_col] = []
                    else:
                        analysis_dict[json_col] = []
                repo_data["analysis"] = analysis_dict
            else:
                repo_data["analysis"] = None

            return repo_data

    def toggle_favorite(self, repo_id: str, notes: str = "", tags: str = "") -> bool:
        """Toggles favorite status. Returns True if now favorited, False if removed."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT repo_id FROM favorites WHERE repo_id = ?", (repo_id,))
            exists = cursor.fetchone()
            if exists:
                cursor.execute("DELETE FROM favorites WHERE repo_id = ?", (repo_id,))
                conn.commit()
                return False
            else:
                cursor.execute("INSERT INTO favorites (repo_id, notes, tags) VALUES (?, ?, ?)", (repo_id, notes, tags))
                conn.commit()
                return True

    def get_favorites(self) -> List[Dict[str, Any]]:
        """Lists all favorited repositories."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
            SELECT
                r.*,
                1 as is_favorite,
                f.notes as favorite_notes,
                f.saved_at,
                a.simple_summary,
                a.why_trending,
                a.quick_verdict,
                a.when_to_select,
                a.install_cmd
            FROM favorites f
            JOIN repositories r ON f.repo_id = r.repo_id
            LEFT JOIN repo_analysis a ON r.repo_id = a.repo_id
            GROUP BY r.repo_id
            ORDER BY f.saved_at DESC
            """)
            rows = cursor.fetchall()
            results = []
            for row in rows:
                item = dict(row)
                item["is_favorite"] = True
                if item.get("when_to_select"):
                    try:
                        item["when_to_select"] = json.loads(item["when_to_select"])
                    except Exception:
                        item["when_to_select"] = []
                results.append(item)
            return results

    def get_available_languages(self, date_str: Optional[str] = None) -> List[str]:
        """Lists languages present in the database."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            if date_str:
                cursor.execute("""
                SELECT DISTINCT language FROM repositories
                WHERE date_collected = ? AND language != 'Unknown' AND language != 'N/A'
                ORDER BY language ASC
                """, (date_str,))
            else:
                cursor.execute("""
                SELECT DISTINCT language FROM repositories
                WHERE language != 'Unknown' AND language != 'N/A'
                ORDER BY language ASC
                """)
            return [row["language"] for row in cursor.fetchall() if row["language"]]

    def log_sync(self, date_str: str, repos_found: int, new_repos: int, status: str, message: str, duration: float):
        """Logs sync execution."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("""
            INSERT INTO sync_history (date_str, repos_found, new_repos, status, message, duration_sec)
            VALUES (?, ?, ?, ?, ?, ?)
            """, (date_str, repos_found, new_repos, status, message, duration))
            conn.commit()

    def get_last_sync(self) -> Optional[Dict[str, Any]]:
        """Returns the most recent sync record."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute("SELECT * FROM sync_history ORDER BY id DESC LIMIT 1")
            row = cursor.fetchone()
            if row:
                return dict(row)
            return None

    def get_daily_digest(self, date_str: Optional[str] = None) -> DailyDigest:
        """Generates a high-level summary of the day's trending repositories."""
        with self.get_connection() as conn:
            cursor = conn.cursor()
            if not date_str:
                cursor.execute("SELECT MAX(date_collected) as max_date FROM repositories WHERE period = 'daily'")
                res = cursor.fetchone()
                date_str = res["max_date"] if res and res["max_date"] else datetime.now().strftime("%Y-%m-%d")

            cursor.execute("SELECT COUNT(*) as count FROM repositories WHERE date_collected = ? AND period = 'daily'", (date_str,))
            total = cursor.fetchone()["count"]

            # Top gainer
            cursor.execute("""
            SELECT r.*, a.simple_summary, a.quick_verdict
            FROM repositories r
            LEFT JOIN repo_analysis a ON r.repo_id = a.repo_id
            WHERE r.date_collected = ? AND r.period = 'daily'
            ORDER BY r.stars_today DESC LIMIT 1
            """, (date_str,))
            top_row = cursor.fetchone()
            top_gainer = dict(top_row) if top_row else None

            # Dominant language
            cursor.execute("""
            SELECT language, COUNT(*) as cnt
            FROM repositories
            WHERE date_collected = ? AND period = 'daily' AND language NOT IN ('Unknown', 'N/A')
            GROUP BY language ORDER BY cnt DESC LIMIT 1
            """, (date_str,))
            lang_row = cursor.fetchone()
            dominant_lang = lang_row["language"] if lang_row else "Python"

            # Dominant category
            cursor.execute("""
            SELECT category, COUNT(*) as cnt
            FROM repositories
            WHERE date_collected = ? AND period = 'daily'
            GROUP BY category ORDER BY cnt DESC LIMIT 1
            """, (date_str,))
            cat_row = cursor.fetchone()
            dominant_cat = cat_row["category"] if cat_row else "ia_agentes"

            # Highlights
            cursor.execute("""
            SELECT name, stars_today, language, category
            FROM repositories
            WHERE date_collected = ? AND period = 'daily'
            ORDER BY stars_today DESC LIMIT 3
            """, (date_str,))
            top_items = cursor.fetchall()
            highlights = [
                f"{r['name']} (+{r['stars_today']:,} estrelas hoje em {r['language']})"
                for r in top_items
            ]

            headline = f"Radar de {date_str}: {total} repositórios em alta. Destaque para {dominant_cat.replace('_', ' ').title()} com forte tração em {dominant_lang}."

            return DailyDigest(
                date=date_str,
                total_repos=total,
                top_gainer=top_gainer,
                dominant_language=dominant_lang,
                dominant_category=dominant_cat,
                headline=headline,
                highlights=highlights
            )
