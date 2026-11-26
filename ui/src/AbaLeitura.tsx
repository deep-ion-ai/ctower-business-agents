import { useEffect, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, Cartao, Secao } from "./componentes";
import { plural } from "./estadoCadastro";
import type { PropsConfigProjeto } from "./ConfigProjeto";
import { numero } from "./formatacao";
import { padraoDoCaminho, tamanhoLegivel, type PreviaProjeto } from "./primeiroUso";

interface Rascunho {
  pastas: string[];
  exclusoes: string[];
  inclusoes: string[];
}

function doProjeto(p: PropsConfigProjeto["projeto"]): Rascunho {
  return {
    pastas: p.pastas.map((x) => x.caminho),
    exclusoes: p.exclusoes,
    inclusoes: p.inclusoes,
  };
}

const igual = (a: Rascunho, b: Rascunho) => JSON.stringify(a) === JSON.stringify(b);

const MSG_ULTIMA_PASTA =
  "Não dá para remover a única pasta do projeto: o agente precisa de ao menos uma pasta para ler. Adicione outra pasta antes, ou descadastre o projeto em Avançado.";

/**
 * Aba “O que é analisado” (#192): o resumo “Será analisado” / “Fica fora” (por
 * linguagem e por motivo, com os segredos em destaque), os itens que ficam fora com
 * “Incluir”, as pastas (adicionar e remover sem descadastrar) e as exclusões em chips.
 * As mudanças ficam num rascunho com prévia; a barra “Alterações não salvas” salva tudo
 * de uma vez e lê o projeto de novo. A última pasta não pode sair.
 */
export function AbaLeitura({
  api,
  projeto,
  aoMudar,
  aoReverificarAmbiente,
  aoPendencia,
}: PropsConfigProjeto) {
  const salvo = doProjeto(projeto);
  const [rascunho, setRascunho] = useState<Rascunho>(salvo);
  const [previa, setPrevia] = useState<PreviaProjeto | null>(null);
  const [lendo, setLendo] = useState(false);
  const [salvando, setSalvando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [avisoPasta, setAvisoPasta] = useState<string | null>(null);
  const [novaExclusao, setNovaExclusao] = useState("");
  const [verFora, setVerFora] = useState(false);
  const pendente = !igual(rascunho, salvo);

  const pendencia = useRef(aoPendencia);
  pendencia.current = aoPendencia;
  useEffect(() => {
    pendencia.current?.(pendente);
  }, [pendente]);

  // A prévia acompanha o rascunho: ao abrir a aba e a cada mudança.
  const chave = JSON.stringify(rascunho);
  useEffect(() => {
    let ativo = true;
    setLendo(true);
    setFalha(null);
    const r = JSON.parse(chave) as Rascunho;
    api
      .previaLeitura({ projeto: projeto.id, ...r })
      .then((p) => ativo && setPrevia(p))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)))
      .finally(() => ativo && setLendo(false));
    return () => {
      ativo = false;
    };
  }, [api, projeto.id, chave]);

  const mudar = (r: Partial<Rascunho>) => {
    setAviso(null);
    setAvisoPasta(null);
    setRascunho((atual) => ({ ...atual, ...r }));
  };

  const adicionarPasta = () => {
    api
      .escolherPasta("Pasta do código do projeto")
      .then((p) => {
        if (p && !rascunho.pastas.includes(p)) mudar({ pastas: [...rascunho.pastas, p] });
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const adicionarExclusao = () => {
    const p = novaExclusao.trim();
    if (!p) return;
    if (!rascunho.exclusoes.includes(p)) mudar({ exclusoes: [...rascunho.exclusoes, p] });
    setNovaExclusao("");
  };

  const salvar = () => {
    setSalvando(true);
    setFalha(null);
    api
      .salvarLeitura({ projeto: projeto.id, ...rascunho })
      .then(() => {
        setAviso("Salvo: o projeto foi lido de novo.");
        aoMudar();
        // As pastas mudam a configuração do Claude que o preparo conferiu.
        if (rascunho.pastas.join("\n") !== salvo.pastas.join("\n")) {
          void aoReverificarAmbiente(projeto.id);
        }
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setSalvando(false));
  };

  // Depois de salvar, o projeto relido passa a ser a referência do rascunho.
  const salvoChave = JSON.stringify(salvo);
  useEffect(() => {
    setRascunho(JSON.parse(salvoChave) as Rascunho);
  }, [salvoChave]);

  const inv = previa?.inventario;
  const segredos = inv?.porMotivo.find((m) => m.chave === "segredo")?.quantidade ?? 0;
  const prefixoDe = (caminho: string) => {
    const gravada = projeto.pastas.find((p) => p.caminho === caminho);
    if (gravada) return gravada.prefixo;
    return previa?.pastas.find((p) => p.caminho === caminho)?.prefixo ?? null;
  };

  return (
    <div className={pendente ? "aba-leitura com-barra" : "aba-leitura"}>
      <div className="resumo-leitura" aria-busy={lendo}>
        <Cartao rotulo="Será analisado" className="resumo-cartao">
          <span className="muted">Será analisado</span>
          <strong className="resumo-numero">
            {inv ? `${numero(inv.analisados)} arquivos` : "…"}
            {inv && <span className="muted"> · {tamanhoLegivel(inv.bytesAnalisados)}</span>}
          </strong>
          {inv && inv.porLinguagem.length > 0 && (
            <span className="muted">
              {inv.porLinguagem.map((l) => `${l.rotulo} ${l.quantidade}`).join(" · ")}
            </span>
          )}
        </Cartao>
        <Cartao rotulo="Fica fora" className="resumo-cartao">
          <span className="muted">Fica fora</span>
          <strong className="resumo-numero">{inv ? plural(inv.ignorados, "arquivo", "arquivos") : "…"}</strong>
          {inv && inv.porMotivo.length > 0 && (
            <span className="muted">
              {inv.porMotivo.map((m, i) => (
                <span key={m.chave}>
                  {i > 0 && " · "}
                  <span className={m.chave === "segredo" ? "attention-text" : undefined}>
                    {m.rotulo} {m.quantidade}
                  </span>
                </span>
              ))}
            </span>
          )}
          {segredos > 0 && (
            <span className="attention-text">
              {segredos === 1
                ? "1 arquivo parece ter segredo e fica fora da análise."
                : `${numero(segredos)} arquivos parecem ter segredos e ficam fora da análise.`}
            </span>
          )}
          {inv && inv.itensIgnorados.length > 0 && (
            <button
              type="button"
              className="link"
              aria-expanded={verFora}
              onClick={() => setVerFora((v) => !v)}
            >
              {verFora ? "Esconder a lista" : "Ver a lista e reincluir itens"}
            </button>
          )}
        </Cartao>
      </div>
      {inv && verFora && (
        <Secao rotulo="Itens que ficam fora">
          <ul className="lista-ignorados">
            {inv.itensIgnorados.map((i) => (
              <li key={i.caminho}>
                <code>
                  {i.caminho}
                  {i.pasta ? "/" : ""}
                </code>{" "}
                <span className="muted">{i.rotulo}</span>
                {i.ajustavel && (
                  <>
                    {" "}
                    <button
                      className="btn btn-sm"
                      type="button"
                      aria-label={`Incluir ${i.caminho} na análise`}
                      onClick={() => {
                        const p = padraoDoCaminho(i.caminho);
                        if (!rascunho.inclusoes.includes(p)) {
                          mudar({ inclusoes: [...rascunho.inclusoes, p] });
                        }
                      }}
                    >
                      Incluir
                    </button>
                  </>
                )}
              </li>
            ))}
          </ul>
          {inv.totalItensIgnorados > inv.itensIgnorados.length && (
            <p className="muted">
              E mais {inv.totalItensIgnorados - inv.itensIgnorados.length} itens.
            </p>
          )}
        </Secao>
      )}

      <Secao
        titulo="Pastas"
        nivel={4}
        rotulo="Pastas do projeto"
        acoes={
          <button className="btn btn-sm" type="button" onClick={adicionarPasta}>
            + Adicionar pasta
          </button>
        }
      >
        <ul className="lista-config">
          {rascunho.pastas.map((caminho) => {
            const gravada = projeto.pastas.find((p) => p.caminho === caminho);
            const prefixo = prefixoDe(caminho);
            const ultima = rascunho.pastas.length === 1;
            return (
              <li key={caminho} className="linha-config linha-pasta">
                <span className="caminho">{caminho}</span>
                <span className="muted">
                  {!gravada ? "nova" : prefixo ? `aparece como ${prefixo}/` : "raiz (sem prefixo)"}
                  {gravada && !gravada.existe && (
                    <strong className="attention-text"> · a pasta não existe mais</strong>
                  )}
                </span>
                <button
                  className="btn btn-sm"
                  type="button"
                  aria-label={`Remover a pasta ${caminho}`}
                  aria-disabled={ultima || undefined}
                  title={ultima ? "O projeto precisa de ao menos uma pasta" : undefined}
                  onClick={() => {
                    // Recusa explicada: um botão desabilitado não diz por quê (#227).
                    if (ultima) {
                      setAvisoPasta(MSG_ULTIMA_PASTA);
                      return;
                    }
                    mudar({ pastas: rascunho.pastas.filter((p) => p !== caminho) });
                  }}
                >
                  Remover
                </button>
              </li>
            );
          })}
        </ul>
        {avisoPasta && <Alerta tom="atencao" role="alert" titulo={avisoPasta} />}
        <p className="muted">
          Remover uma pasta não apaga nada no computador: ela só deixa de ser lida. As pastas que
          ficam guardam o prefixo, e as evidências das análises continuam apontando para o mesmo
          lugar.
        </p>
      </Secao>

      <Secao titulo="Deixar de fora também" nivel={4} rotulo="Suas exclusões">
        <p className="muted">Um padrão por item, como no .gitignore.</p>
        <div className="exclusoes-corpo">
          {rascunho.exclusoes.length > 0 && (
            <ul className="chips-exclusao">
              {rascunho.exclusoes.map((e) => (
                <li key={e} className="chip-exclusao">
                  <code>{e}</code>
                  <button
                    type="button"
                    aria-label={`Remover a exclusão ${e}`}
                    onClick={() => mudar({ exclusoes: rascunho.exclusoes.filter((x) => x !== e) })}
                  >
                    <span aria-hidden="true">✕</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
          <form
            className="nova-exclusao"
            onSubmit={(ev) => {
              ev.preventDefault();
              adicionarExclusao();
            }}
          >
            <input
              type="text"
              className="caminho"
              aria-label="Padrão para deixar de fora"
              placeholder="ex.: **/legado/**"
              spellCheck={false}
              value={novaExclusao}
              onChange={(e) => setNovaExclusao(e.target.value)}
            />
            <button className="btn btn-sm" type="submit" disabled={!novaExclusao.trim()}>
              Adicionar
            </button>
          </form>
          {rascunho.inclusoes.length > 0 && (
            <div role="group" aria-label="Reincluídos">
              <p className="muted">Reincluídos por você:</p>
              <ul className="chips-exclusao">
                {rascunho.inclusoes.map((x) => (
                  <li key={x} className="chip-exclusao">
                    <code>{x}</code>
                    <button
                      type="button"
                      aria-label={`Tirar a reinclusão de ${x}`}
                      onClick={() =>
                        mudar({ inclusoes: rascunho.inclusoes.filter((y) => y !== x) })
                      }
                    >
                      <span aria-hidden="true">✕</span>
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      </Secao>

      {falha && <Alerta tom="erro" titulo={falha} />}
      {aviso && !pendente && (
        <p className="ok-text" role="status">
          {aviso}
        </p>
      )}
      {pendente && (
        <div className="barra-pendente" role="region" aria-label="Alterações não salvas">
          <span>Alterações não salvas: a prévia acima já mostra o resultado.</span>
          <button
            className="btn btn-sm"
            type="button"
            disabled={salvando}
            onClick={() => {
              setRascunho(salvo);
              setFalha(null);
            }}
          >
            Descartar
          </button>
          <button
            className="btn btn-sm btn-primary"
            type="button"
            disabled={salvando}
            onClick={salvar}
          >
            Salvar e ler o projeto de novo
          </button>
        </div>
      )}
    </div>
  );
}
