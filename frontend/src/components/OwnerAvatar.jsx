import React, { useEffect, useState } from "react";

// Cache por sessão: o mesmo autor aparece em muitos cards.
const hfCache = new Map();

async function hfAvatar(author) {
  if (!hfCache.has(author)) {
    hfCache.set(
      author,
      (async () => {
        // Autor do Hugging Face pode ser organização ou usuário; cada um tem um endpoint.
        for (const kind of ["organizations", "users"]) {
          try {
            const res = await fetch(
              `https://huggingface.co/api/${kind}/${encodeURIComponent(author)}/avatar`,
            );
            if (res.ok) {
              const { avatarUrl } = await res.json();
              if (avatarUrl) return avatarUrl;
            }
          } catch {
            // sem rede: cai na inicial
          }
        }
        return "";
      })(),
    );
  }
  return hfCache.get(author);
}

// Só a foto do dono, sem o nome visível (o nome fica no alt/title para leitor de tela e tooltip).
export default function OwnerAvatar({ owner, source, size = 40 }) {
  const isHf = source === "hf";
  const githubUrl =
    owner && !isHf
      ? `https://github.com/${encodeURIComponent(owner)}.png?size=${size * 2}`
      : "";
  const [hfUrl, setHfUrl] = useState("");
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (!isHf || !owner) return;
    let active = true;
    hfAvatar(owner).then((url) => {
      if (active) setHfUrl(url);
    });
    return () => {
      active = false;
    };
  }, [isHf, owner]);

  const src = isHf ? hfUrl : githubUrl;
  const style = { width: size, height: size };
  if (!owner) return null;
  if (!src || failed) {
    return (
      <span
        className="owner-avatar-fallback"
        style={style}
        title={owner}
        aria-label={owner}
        role="img"
      >
        {owner.slice(0, 1).toUpperCase()}
      </span>
    );
  }
  return (
    <img
      className="owner-avatar-photo"
      style={style}
      src={src}
      alt={owner}
      title={owner}
      loading="lazy"
      onError={() => setFailed(true)}
    />
  );
}
