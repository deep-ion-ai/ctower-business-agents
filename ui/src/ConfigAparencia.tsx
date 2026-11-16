import { useState } from "react";
import { THEME_PREFS, useTheme, type ThemePref } from "./shell/theme";

const DESCRICAO: Record<ThemePref, string> = {
  system: "Acompanha o tema claro ou escuro do sistema operacional.",
  light: "Fundo claro, para ambientes iluminados.",
  dark: "Fundo escuro, mais confortável com pouca luz.",
};

/** Miniatura da tela no tema do cartão; “Do sistema” mostra metade clara e metade escura. */
function Miniatura({ pref }: { pref: ThemePref }) {
  return (
    <span className="tema-miniatura" data-tema={pref} aria-hidden="true">
      <span className="tema-miniatura-lado" data-tema={pref === "system" ? "light" : pref}>
        <span className="tema-miniatura-barra" />
        <span className="tema-miniatura-linha" />
        <span className="tema-miniatura-linha curta" />
      </span>
      {pref === "system" && (
        <span className="tema-miniatura-lado" data-tema="dark">
          <span className="tema-miniatura-barra" />
          <span className="tema-miniatura-linha" />
          <span className="tema-miniatura-linha curta" />
        </span>
      )}
    </span>
  );
}

/**
 * Aparência (#85, #191): três cartões com miniatura (Do sistema, Claro, Escuro), o
 * mesmo tema da barra de título. A escolha vale na hora, com uma confirmação discreta.
 */
export function ConfigAparencia() {
  const [pref, mudar] = useTheme();
  const [aplicado, setAplicado] = useState<ThemePref | null>(null);
  const rotulo = THEME_PREFS.find((t) => t.pref === aplicado)?.rotulo;

  return (
    <section className="config-secao" aria-labelledby="config-aparencia">
      <header className="config-secao-topo">
        <h2 id="config-aparencia">Aparência</h2>
        <p className="muted">Muda na hora e vale para este computador.</p>
      </header>
      <fieldset className="cartoes-tema">
        <legend className="sr-only">Tema</legend>
        {THEME_PREFS.map((t) => (
          <label key={t.pref} className="cartao-tema" data-escolhido={pref === t.pref}>
            <input
              type="radio"
              name="tema"
              value={t.pref}
              checked={pref === t.pref}
              onChange={() => {
                mudar(t.pref);
                setAplicado(t.pref);
              }}
            />
            <Miniatura pref={t.pref} />
            <strong>{t.rotulo}</strong>
            <span className="muted">{DESCRICAO[t.pref]}</span>
          </label>
        ))}
      </fieldset>
      <p className="ok-text confirmacao-tema" role="status">
        {rotulo ? `Tema “${rotulo}” aplicado.` : ""}
      </p>
    </section>
  );
}
