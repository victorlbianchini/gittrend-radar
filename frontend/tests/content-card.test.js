import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { createServer } from 'vite';

let server;
// React and renderToStaticMarkup MUST come from the same Vite module graph as the
// components under test. Importing them from the Node side yields a second React
// instance whose hook dispatcher is null during SSR (Invalid hook call).
let React;
let renderToStaticMarkup;
let ContentCard;
let cardDetails;
let RepoCard;
before(async () => {
  server = await createServer({ server: { middlewareMode: true, ws: false, hmr: false }, appType: 'custom' });
  const kit = await server.ssrLoadModule('/tests/react-ssr-kit.js');
  React = kit.React;
  renderToStaticMarkup = kit.renderToStaticMarkup;
  ContentCard = (await server.ssrLoadModule('/src/components/ContentCard.jsx')).default;
  ({ cardDetails } = await server.ssrLoadModule('/src/components/cardDetails.js'));
  RepoCard = (await server.ssrLoadModule('/src/components/RepoCard.jsx')).default;
});
after(async () => {
  await server?.close();
  process.exit(0);
});

const fixtures = [
  { key: 'github:demo/repo', title: 'Repository', url: 'https://github.com/demo/repo', source: 'GitHub', repo: { owner: 'demo', stars: 12 } },
  { key: 'hf:demo/model', title: 'Model', url: 'https://huggingface.co/demo/model', source: 'Hugging Face', model: { author: 'demo', likes: 2 } },
  { key: 'skill:demo/debug', title: 'Model Debugging', url: 'https://github.com/demo/skills', source: 'GitHub', skill: { repo: 'demo/skills' } },
];
const render = (item, record = { status: 'ready', summary: 'Resumo curto em português.' }, extra = {}) => renderToStaticMarkup(React.createElement(ContentCard, { item, record, onDownload() {}, ...extra }));

test('repositories, models and skills render the same simple summary and direct download structure', () => {
  for (const item of fixtures) {
    const html = render(item);
    assert.match(html, /<article class="repo-card-luxury /);
    assert.match(html, /aria-label="Resumo"/);
    assert.match(html, /Resumo curto em português\./);
    assert.match(html, /<span>Baixar<\/span>/);
    assert.match(html, new RegExp(`href="${item.url}"`));
    assert.match(html, /<button type="button" class="btn-details-glass" aria-expanded="false"[^>]*>.*Detalhes<\/button>/);
    assert.doesNotMatch(html, /IA EXPLICA|Detalhes e download|aria-label="Detalhes"|Motor:|Ver detalhes|terminal-body/);
    assert.equal((html.match(/aria-label="Resumo"/g) || []).length, 1);
    assert.doesNotMatch(html, /0 estrelas|0 downloads|0 instalações/);
  }
});

test('models default to files without weights and expose the full download as an explicit option', () => {
  const html = render(fixtures[1]);
  assert.match(html, /<option value="light" selected="">Arquivos sem pesos<\/option>/);
  assert.match(html, /<option value="full">Completo com pesos/);
  assert.doesNotMatch(html, /<option value="full" selected/);
});

test('pending and failed summaries show honest compact statuses and an optional retry', () => {
  assert.match(render(fixtures[0], { status: 'pending' }), /Resumindo…/);
  const failed = render(fixtures[2], { status: 'error', error: 'sensitive internal endpoint' }, { onRetry() {} });
  assert.match(failed, /Resumo indisponível\./);
  assert.match(failed, /Tentar novamente/);
  assert.doesNotMatch(failed, /sensitive internal endpoint/);
});

test('downloaded cards expose the known folder and disable duplicate download', () => {
  const html = render({ ...fixtures[2], skill: { ...fixtures[2].skill, is_downloaded: true, downloaded_path: 'E:/skills/debug' } }, undefined, { onOpenFolder() {} });
  assert.match(html, /disabled=""/);
  assert.match(html, /<span>Baixado<\/span>/);
  assert.match(html, /Abrir pasta/);
});

test('library wrapper preserves summary status and the same card structure', () => {
  const html = renderToStaticMarkup(React.createElement(RepoCard, {
    repo: { repo_id: 'demo/repo', name: 'Repo', owner: 'demo', url: fixtures[0].url },
    summaryRecord: { status: 'error' }, onRetry() {}, onCloneClick() {},
  }));
  assert.match(html, /Resumo indisponível\./);
  assert.match(html, /Tentar novamente/);
  assert.match(html, /<span>Baixar<\/span>/);
  assert.match(html, /aria-expanded="false"/);
  assert.doesNotMatch(html, /IA EXPLICA|Detalhes e download/);
});

test('active model jobs block duplicate downloads and never offer an incomplete folder', () => {
  const html = render(fixtures[1], undefined, { onOpenFolder() {}, downloadJob: { finished: false, folder_path: 'E:/partial', current_file: 'weights.safetensors' } });
  assert.match(html, /<button[^>]*class="btn-download-cyber" disabled=""/);
  assert.match(html, /Baixando…/);
  assert.match(html, /weights.safetensors/);
  assert.doesNotMatch(html, /Abrir pasta|Download concluído/);
});

test('finished model jobs expose failures or a successful folder and retain full-download selection', () => {
  const failure = render(fixtures[1], undefined, { downloadJob: { finished: true, success: false, message: 'Sem espaço no disco.', folder_path: 'E:/partial' }, onOpenFolder() {} });
  assert.match(failure, /role="alert"[^>]*>Sem espaço no disco\./);
  assert.doesNotMatch(failure, /Abrir pasta|Download concluído/);
  const success = render(fixtures[1], undefined, { downloadJob: { finished: true, success: true, folder_path: 'E:/model' }, onOpenFolder() {} });
  assert.match(success, /Abrir pasta/);
  assert.match(success, /Download concluído/);
  assert.match(success, /<option value="full">Completo com pesos/);
  assert.doesNotMatch(success, /disabled=""/);
});

test('details only expose delivered facts and skip text already shown as the summary', () => {
  const empty = cardDetails({ key: 'skill:x', title: 'X', skill: { repo: '', sources: [] } }, { status: 'pending' });
  assert.deepEqual(empty, { facts: [], texts: [], lists: [], installCmd: undefined });

  const repo = cardDetails({
    key: 'github:demo/repo', title: 'Repo', description: 'Original', published_at: '2026-10-15',
    repo: { repo_id: 'demo/repo', language: 'Rust', stars_today: 978, analysis: { why_trending: 'Lançamento novo', key_features: ['Rápido', 'Rápido'], when_to_avoid: [], install_cmd: 'git clone https://github.com/demo/repo' } },
  }, { status: 'ready', description_pt: 'Resumo curto', summary: 'Resumo curto' }, 'Resumo curto');
  assert.deepEqual(repo.facts, [['Linguagem', 'Rust'], ['Estrelas hoje', '+978'], ['Publicado', '2026-10-15'], ['Identificador', 'demo/repo']]);
  assert.deepEqual(repo.texts, [['Por que está em alta', 'Lançamento novo']]);
  assert.deepEqual(repo.lists, [['Principais recursos', ['Rápido', 'Rápido']]]);
  assert.equal(repo.installCmd, 'git clone https://github.com/demo/repo');

  const model = cardDetails({ key: 'hf:g/e', title: 'e', description: 'Embeddings', model: { id: 'g/e', task_label: 'Embeddings', likes: 0 } }, null);
  assert.deepEqual(model.facts, [['Tarefa', 'Embeddings'], ['Identificador', 'g/e']]);
  assert.deepEqual(model.texts, [['Descrição', 'Embeddings']]);
});

test('owner appears only as a photo and the translated summary comes before the AI summary', () => {
  const item = { ...fixtures[0], description: 'Original in English', repo: { owner: 'demo', stars: 12, stars_today: 30 } };
  const html = render(item, { status: 'ready', description_pt: 'Descrição traduzida.', summary: 'Resumo curto da IA.', model: 'qwen3:0.6b' });
  // Nome do dono so como alt/title da foto, nunca como texto visivel.
  assert.match(html, /<img class="owner-avatar-photo"[^>]*alt="demo"/);
  assert.doesNotMatch(html, />@?demo</);
  const translated = html.indexOf('Descrição traduzida.');
  const ai = html.indexOf('Resumo curto da IA.');
  assert.ok(translated > 0 && ai > translated, 'tradução antes do resumo da IA');
  assert.match(html, /Resumo traduzido/);
  assert.match(html, /Resumo da IA/);
  assert.match(html, /\+30<\/strong> hoje/);
  assert.equal((html.match(/Descrição traduzida\./g) || []).length, 1);
  // Sem tradução ainda: rotula honestamente como descrição original.
  assert.match(render(item, { status: 'pending' }), /Descrição original/);
});

test('title shows only the project name, never owner/name', () => {
  const item = { ...fixtures[0], title: 'demo/repo', description_pt: 'Descrição já traduzida.', repo: { owner: 'demo', name: 'repo', stars: 5 } };
  const html = render(item, { status: 'pending' });
  assert.match(html, /<h3 class="card-v2-title" title="demo\/repo">repo<\/h3>/);
  assert.match(html, /Resumo traduzido/);
  assert.doesNotMatch(html, /Descrição original/);
});
