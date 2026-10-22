const present = (value) =>
  Array.isArray(value)
    ? value.length > 0
    : typeof value === "string"
      ? value.trim() !== ""
      : value != null;

// Only facts the source actually delivered; missing fields stay hidden instead of being invented.
// `shown`: texto(s) que o card ja exibe (traducao e/ou resumo da IA), para nao repetir nos detalhes.
export function cardDetails(item, record, shown = "") {
  const { repo, model, skill } = item;
  const alreadyShown = new Set([].concat(shown).filter(Boolean));
  const analysis = repo?.analysis;
  const facts = [
    ["Tarefa", model?.task_label],
    ["Linguagem", repo?.language],
    [
      "Estrelas hoje",
      repo?.stars_today > 0
        ? `+${repo.stars_today.toLocaleString("pt-BR")}`
        : null,
    ],
    ["Publicado", item.published_at],
    ["Visto pela primeira vez", item.first_seen || repo?.first_seen],
    ["Repositório", skill?.repo],
    ["Pasta da skill", skill?.path],
    ["Fontes", skill?.sources?.join(", ")],
    ["Identificador", model?.id || repo?.repo_id || skill?.id],
  ].filter(([, value]) => present(value));
  const texts = [
    ["Descrição", record?.description_pt || item.description],
    ["Por que está em alta", analysis?.why_trending || repo?.why_trending],
    ["Veredito", analysis?.quick_verdict || repo?.quick_verdict],
    ["Para quem é", analysis?.target_audience],
  ].filter(([, value]) => present(value) && !alreadyShown.has(value));
  const lists = [
    ["Principais recursos", analysis?.key_features],
    ["Quando usar", analysis?.when_to_select || repo?.when_to_select],
    ["Quando evitar", analysis?.when_to_avoid],
    ["Tecnologias", analysis?.tech_stack],
  ].filter(([, value]) => present(value));
  const installCmd = analysis?.install_cmd || repo?.install_cmd;
  return { facts, texts, lists, installCmd };
}
