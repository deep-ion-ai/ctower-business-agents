import { useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, Secao } from "./componentes";
import type { PropsConfigProjeto } from "./ConfigProjeto";

/**
 * Dados pessoais do texto da demanda na exportação e na publicação (#454): CPF, CNPJ,
 * e-mail e telefone de "Demanda como veio" e "Demanda como entendida" saem mascarados.
 * Preferência do projeto, ligada por padrão; a base local guarda sempre o original.
 */
export function MascaraDaDemanda({
  api,
  projeto,
  aoMudar,
}: Pick<PropsConfigProjeto, "api" | "projeto" | "aoMudar">) {
  const [ligado, setLigado] = useState(projeto.mascararDemanda ?? true);
  const [salvando, setSalvando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const salvar = api.salvarMascararDemanda;

  const alternar = (valor: boolean) => {
    if (!salvar) return;
    setSalvando(true);
    setFalha(null);
    salvar(projeto.id, valor)
      .then((gravado) => {
        setLigado(gravado);
        aoMudar();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setSalvando(false));
  };

  return (
    <Secao titulo="Dados pessoais da demanda" nivel={4} className="mascara-demanda">
      <label className="check">
        <input
          type="checkbox"
          checked={ligado}
          disabled={salvando || !salvar}
          onChange={(e) => alternar(e.target.checked)}
        />{" "}
        Mascarar CPF, CNPJ, e-mail e telefone do texto da demanda ao exportar e publicar
      </label>
      <p className="muted">
        Vale para “Demanda como veio” e “Demanda como entendida”. O texto original continua na
        base; só o documento exportado ou publicado sai mascarado.
      </p>
      {falha && <Alerta tom="erro" titulo={falha} />}
    </Secao>
  );
}
