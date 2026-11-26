import { Icon } from "./shell/icons";

/**
 * Ícone `</> N` que mostra ou esconde as evidências no código de uma afirmação ou
 * sugestão (#130, #404).
 */
export function BotaoCodigo({
  quantidade,
  aberto,
  aoAlternar,
}: {
  quantidade: number;
  aberto: boolean;
  aoAlternar: () => void;
}) {
  const rotulo = `${aberto ? "Ocultar" : "Ver"} ${
    quantidade === 1 ? "a evidência" : `as ${quantidade} evidências`
  } no código`;
  return (
    <button
      type="button"
      className="btn-icone btn-acao btn-codigo"
      aria-label={rotulo}
      title={rotulo}
      aria-expanded={aberto}
      onClick={aoAlternar}
    >
      <Icon name="code" />
      <span className="contador-codigo">{quantidade}</span>
    </button>
  );
}
