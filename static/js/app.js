/**
 * GitTrend Radar - Vue 3 Reactive Application
 * Strictly complies with ZERO DOM Rule: No direct DOM queries or manipulations.
 */

const { createApp, ref, computed, onMounted, watch } = Vue;

createApp({
  setup() {
    // --- State ---
    const repos = ref([]);
    const favorites = ref([]);
    const digest = ref(null);
    const status = ref({
      is_syncing: false,
      last_sync_date: null,
      last_sync_time: null,
      total_repos_today: 0,
      message: 'Carregando...',
      current_step: ''
    });
    const languages = ref([]);
    const selectedRepo = ref(null);
    
    // UI Filters & Tabs
    const activeTab = ref('trends'); // 'trends' | 'favorites'
    const selectedPeriod = ref('daily'); // 'daily' | 'weekly' | 'monthly'
    const selectedCategory = ref('all');
    const selectedLanguage = ref('all');
    const searchQuery = ref('');
    const sortBy = ref('stars_today');
    
    // Loading Flags
    const isLoading = ref(true);
    const isDetailLoading = ref(false);
    const isAnalyzing = ref(false);
    const copyFeedback = ref('');

    // Predefined Categories
    const categories = [
      { id: 'all', label: 'Todas as Categorias', icon: '⚡' },
      { id: 'ia_agentes', label: 'IA & Agentes Autônomos', icon: '🤖' },
      { id: 'frontend_ui', label: 'Frontend & Design Systems', icon: '🎨' },
      { id: 'backend_api', label: 'Backend & APIs', icon: '⚙️' },
      { id: 'devops_cli', label: 'DevOps & Terminal CLI', icon: '🚀' },
      { id: 'security', label: 'Segurança & Reversa', icon: '🛡️' },
      { id: 'data_eng', label: 'Engenharia de Dados', icon: '📊' }
    ];

    // Language Color Mapping
    const langColors = {
      'Python': '#3572A5',
      'TypeScript': '#3178C6',
      'JavaScript': '#F7DF1E',
      'Rust': '#DEA584',
      'Go': '#00ADD8',
      'C++': '#F34B7D',
      'C': '#555555',
      'HTML': '#E34C26',
      'CSS': '#563D7C',
      'Jupyter Notebook': '#DA5B0B',
      'Shell': '#89E051',
      'Unknown': '#64748B'
    };

    function getLangColor(lang) {
      return langColors[lang] || '#00F0FF';
    }

    // --- API Calls ---

    async function fetchStatus() {
      try {
        const res = await fetch('/api/status');
        if (res.ok) {
          status.value = await res.json();
        }
      } catch (err) {
        console.error('Error fetching status:', err);
      }
    }

    async function fetchDigest() {
      try {
        const res = await fetch('/api/digest');
        if (res.ok) {
          digest.value = await res.json();
        }
      } catch (err) {
        console.error('Error fetching digest:', err);
      }
    }

    async function fetchLanguages() {
      try {
        const res = await fetch('/api/languages');
        if (res.ok) {
          const data = await res.json();
          languages.value = data.languages || [];
        }
      } catch (err) {
        console.error('Error fetching languages:', err);
      }
    }

    async function fetchTrends() {
      isLoading.value = true;
      try {
        const params = new URLSearchParams({
          period: selectedPeriod.value,
          category: selectedCategory.value,
          language: selectedLanguage.value,
          sort_by: sortBy.value,
          search: searchQuery.value
        });
        const res = await fetch(`/api/trends?${params.toString()}`);
        if (res.ok) {
          const data = await res.json();
          repos.value = data.repositories || [];
        }
      } catch (err) {
        console.error('Error fetching trends:', err);
      } finally {
        isLoading.value = false;
      }
    }

    async function fetchFavorites() {
      try {
        const res = await fetch('/api/favorites');
        if (res.ok) {
          const data = await res.json();
          favorites.value = data.favorites || [];
        }
      } catch (err) {
        console.error('Error fetching favorites:', err);
      }
    }

    async function triggerSync(force = false) {
      try {
        status.value.is_syncing = true;
        status.value.message = 'Iniciando atualização...';
        await fetch(`/api/sync?force=${force}&since=${selectedPeriod.value}`, { method: 'POST' });
        
        // Poll status every 1.5 seconds until finished
        const pollInterval = setInterval(async () => {
          await fetchStatus();
          if (!status.value.is_syncing) {
            clearInterval(pollInterval);
            await fetchTrends();
            await fetchDigest();
            await fetchLanguages();
          }
        }, 1500);
      } catch (err) {
        console.error('Error triggering sync:', err);
        status.value.is_syncing = false;
      }
    }

    async function openDetail(repo) {
      selectedRepo.value = repo;
      isDetailLoading.value = true;
      try {
        const res = await fetch(`/api/repo/${repo.owner}/${repo.name}`);
        if (res.ok) {
          const fullData = await res.json();
          selectedRepo.value = fullData;
        }
      } catch (err) {
        console.error('Error loading repo details:', err);
      } finally {
        isDetailLoading.value = false;
      }
    }

    function closeDetail() {
      selectedRepo.value = null;
    }

    async function toggleFavorite(repo) {
      try {
        const res = await fetch(`/api/favorites/${repo.owner}/${repo.name}`, { method: 'POST' });
        if (res.ok) {
          const data = await res.json();
          repo.is_favorite = data.is_favorite;
          if (selectedRepo.value && selectedRepo.value.repo_id === repo.repo_id) {
            selectedRepo.value.is_favorite = data.is_favorite;
          }
          await fetchFavorites();
        }
      } catch (err) {
        console.error('Error toggling favorite:', err);
      }
    }

    async function reanalyze(repo, mode = 'heuristic') {
      if (!repo) return;
      isAnalyzing.value = true;
      try {
        const res = await fetch(`/api/analyze/${repo.owner}/${repo.name}?mode=${mode}`, { method: 'POST' });
        if (res.ok) {
          const data = await res.json();
          if (selectedRepo.value && selectedRepo.value.repo_id === repo.repo_id) {
            selectedRepo.value.analysis = data.analysis;
          }
          // Update card in list
          const found = repos.value.find(r => r.repo_id === repo.repo_id);
          if (found) {
            found.simple_summary = data.analysis.simple_summary;
            found.why_trending = data.analysis.why_trending;
            found.when_to_select = data.analysis.when_to_select;
            found.quick_verdict = data.analysis.quick_verdict;
          }
        }
      } catch (err) {
        console.error('Error reanalyzing:', err);
      } finally {
        isAnalyzing.value = false;
      }
    }

    function copyToClipboard(text) {
      if (!text) return;
      // Strip markdown backticks if copying full block
      const clean = text.replace(/^```[a-z]*\n/i, '').replace(/\n```$/, '');
      navigator.clipboard.writeText(clean).then(() => {
        copyFeedback.value = 'Copiado!';
        setTimeout(() => { copyFeedback.value = ''; }, 2000);
      });
    }

    function exportMarkdown() {
      window.open('/api/export', '_blank');
    }

    // --- Search Debouncing & Watchers ---
    let searchTimeout = null;
    watch(searchQuery, () => {
      clearTimeout(searchTimeout);
      searchTimeout = setTimeout(() => {
        fetchTrends();
      }, 350);
    });

    watch([selectedPeriod, selectedCategory, selectedLanguage, sortBy], () => {
      fetchTrends();
    });

    // --- Initial Load ---
    onMounted(async () => {
      await fetchStatus();
      await fetchDigest();
      await fetchLanguages();
      await fetchTrends();
      await fetchFavorites();

      // If database is empty or never synced, trigger initial sync automatically
      if (repos.value.length === 0 && !status.value.is_syncing) {
        triggerSync(false);
      }
    });

    return {
      repos,
      favorites,
      digest,
      status,
      languages,
      selectedRepo,
      activeTab,
      selectedPeriod,
      selectedCategory,
      selectedLanguage,
      searchQuery,
      sortBy,
      categories,
      isLoading,
      isDetailLoading,
      isAnalyzing,
      copyFeedback,
      getLangColor,
      triggerSync,
      openDetail,
      closeDetail,
      toggleFavorite,
      reanalyze,
      copyToClipboard,
      exportMarkdown
    };
  }
}).mount('#app');
