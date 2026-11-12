import { useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import { FormRepositorios } from "./FormRepositorios";
import { SEM_REPOSITORIOS, validarRepositorios, type Repositorios } from "./repositorios";
import {
  padraoDoCaminho,
  tamanhoLegivel,
  type ApiPrimeiroUso,
  type PedidoProjeto,
  type PreviaProjeto,
  type ResumoInventario,
  type ProjetoCadastrado,
} from "./primeiroUso";

type Api = Pick<ApiPrimeiroUso, "previaProjeto" | "cadastrarProjeto" | "escolherPasta">;

function linhas(texto: string): string[] {
  return texto
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}

/** Os repositórios não mudam o inventário: a prévia vale mesmo se eles mudarem. */
function chaveDoPedido(p: PedidoProjeto): string {
  return JSON.stringify({ ...p, repositorios: undefined });
}

/**
 * Cadastro de um projeto (passo do primeiro uso e das configurações): nome, uma ou mais
 * pastas, a prévia do que será analisado e do que fica fora (binários, dependências,
 * possíveis segredos) e o ajuste das exclusões antes de cadastrar. O cadastro grava o
 * inventário e cria o projeto na base.
 */
export function CadastroProjeto({
  api,
  aoConcluir,
}: {
  api: Api;
  aoConcluir: (projeto: ProjetoCadastrado) => void;
}) {
  const [nome, setNome] = useState("");
  const [pastas, setPastas] = useState<string[]>([""]);
  const [exclusoes, setExclusoes] = useState("");
  const [inclusoes, setInclusoes] = useState<string[]>([]);
  const [repositorios, setRepositorios] = useState<Repositorios>(SEM_REPOSITORIOS);
  const [previa, setPrevia] = useState<{
    chave: string;
    dados: PreviaProjeto;
  } | null>(null);
  const [ocupado, setOcupado] = useState<"previa" | "cadastro" | null>(null);
  const [falha, setFalha] = useState<string | null>(null);

  const pedido: PedidoProjeto = {
    nome: nome.trim(),
    pastas: pastas.map((p) => p.trim()).filter(Boolean),
    exclusoes: linhas(exclusoes),
    inclusoes,
    repositorios,
  };
  const completo = pedido.nome !== "" && pedido.pastas.length > 0;
  const atualizada = previa !== null && previa.chave === chaveDoPedido(pedido);

  const verPrevia = (p: PedidoProjeto = pedido) => {
    const invalido = validarRepositorios(p.repositorios ?? SEM_REPOSITORIOS);
    if (invalido) {
      setFalha(invalido);
      return;
    }
    setOcupado("previa");
    setFalha(null);
    api
      .previaProjeto(p)
      .then((dados) => setPrevia({ chave: chaveDoPedido(p), dados }))
      .catch((e: unknown) => {
        setPrevia(null);
        setFalha(mensagemDe(e));
      })
      .finally(() => setOcupado(null));
  };

  const cadastrar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!atualizada) {
      verPrevia();
      return;
    }
    setOcupado("cadastro");
    setFalha(null);
    api
      .cadastrarProjeto(pedido)
      .then(aoConcluir)
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setOcupado(null);
      });
  };

  const reincluir = (caminho: string) => {
    const padrao = padraoDoCaminho(caminho);
    if (inclusoes.includes(padrao)) return;
    const novas = [...inclusoes, padrao];
    setInclusoes(novas);
    verPrevia({ ...pedido, inclusoes: novas });
  };

  const tirarInclusao = (padrao: string) => {
    const novas = inclusoes.filter((x) => x !== padrao);
    setInclusoes(novas);
    verPrevia({ ...pedido, inclusoes: novas });
  };

  const escolher = (i: number) => {
    api
      .escolherPasta("Pasta do código do projeto", pastas[i]?.trim() || undefined)
      .then((p) => p && setPastas((l) => l.map((x, j) => (j === i ? p : x))))
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  return (
    <form className="passo-assistente" onSubmit={cadastrar} aria-label="Cadastrar projeto">
      <h2>Qual projeto você vai analisar?</h2>
      <p>
        Dê um nome ao projeto e indique as pastas do código (um clone do repositório, de preferência
        com acesso só de leitura). O app não grava nada nessas pastas.
      </p>
      <label>
        Nome do projeto{" "}
        <input
          value={nome}
          onChange={(e) => setNome(e.target.value)}
          placeholder="Ex.: Faturamento Exemplo"
        />
      </label>
      <fieldset className="pastas-projeto">
        <legend>Pastas do código</legend>
        {pastas.map((p, i) => (
          <div key={i} className="linha-pasta">
            <label>
              {i === 0 ? "Pasta principal" : `Pasta ${i + 1}`} (caminho completo){" "}
              <input
                className="caminho"
                value={p}
                spellCheck={false}
                onChange={(e) => setPastas((l) => l.map((x, j) => (j === i ? e.target.value : x)))}
              />
            </label>{" "}
            <button className="btn" type="button" onClick={() => escolher(i)}>
              Escolher pasta…
            </button>
            {pastas.length > 1 && (
              <button
                className="btn"
                type="button"
                aria-label={`Remover a pasta ${i + 1}`}
                onClick={() => setPastas((l) => l.filter((_, j) => j !== i))}
              >
                Remover
              </button>
            )}
          </div>
        ))}
        <button className="btn" type="button" onClick={() => setPastas((l) => [...l, ""])}>
          Acrescentar outra pasta
        </button>
      </fieldset>
      <FormRepositorios valor={repositorios} aoMudar={setRepositorios} />

      <details className="ajuste-exclusoes">
        <summary>Ajustar o que fica fora</summary>
        <label>
          Excluir também (um padrão por linha, como no .gitignore: <code>docs/antigos/</code>,{" "}
          <code>*.log</code>)
          <textarea
            rows={3}
            value={exclusoes}
            onChange={(e) => setExclusoes(e.target.value)}
            spellCheck={false}
          />
        </label>
        {inclusoes.length > 0 && (
          <div role="group" aria-label="Reincluídos">
            <p>Reincluídos por você (ficam visíveis para a IA):</p>
            <ul>
              {inclusoes.map((x) => (
                <li key={x}>
                  <code>{x}</code>{" "}
                  <button
                    className="btn"
                    type="button"
                    aria-label={`Tirar a reinclusão de ${x}`}
                    onClick={() => tirarInclusao(x)}
                  >
                    Desfazer
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </details>

      {previa && (
        <Previa
          previa={previa.dados}
          desatualizada={!atualizada}
          aoReincluir={reincluir}
          ocupado={ocupado !== null}
        />
      )}
      {ocupado === "cadastro" && (
        <div role="status" className="progresso">
          <progress aria-label="Inventário do projeto" /> Cadastrando e inventariando as pastas do
          projeto…
        </div>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button
          className="btn"
          type="button"
          onClick={() => verPrevia()}
          disabled={!completo || ocupado !== null}
        >
          {previa ? "Atualizar a prévia" : "Ver o que será analisado"}
        </button>
        <button
          type="submit"
          className="btn btn-primary"
          disabled={!atualizada || ocupado !== null || previa?.dados.jaCadastrado === true}
        >
          Cadastrar o projeto
        </button>
      </div>
    </form>
  );
}

function Previa({
  previa,
  desatualizada,
  aoReincluir,
  ocupado,
}: {
  previa: PreviaProjeto;
  desatualizada: boolean;
  aoReincluir: (caminho: string) => void;
  ocupado: boolean;
}) {
  return (
    <section className="previa-projeto" aria-label="Prévia do inventário">
      {previa.jaCadastrado && (
        <p className="error-text" role="alert">
          Já existe um projeto cadastrado com o nome “{previa.nome}”. Use outro nome.
        </p>
      )}
      {desatualizada && (
        <p className="attention-text" role="status">
          Você mudou o cadastro depois da prévia. Atualize a prévia antes de cadastrar.
        </p>
      )}
      <ul className="muted">
        {previa.pastas.map((p) => (
          <li key={p.caminho}>
            {p.caminho}
            {p.prefixo && ` (aparece como ${p.prefixo}/)`}
            {p.git && ` · git${p.branch ? `, branch ${p.branch}` : ""}`}
          </li>
        ))}
      </ul>
      <ResumoDoInventario
        inventario={previa.inventario}
        aoReincluir={aoReincluir}
        ocupado={ocupado}
      />
    </section>
  );
}

/** O que será analisado (por linguagem) e o que fica fora (por motivo e item a item). */
export function ResumoDoInventario({
  inventario,
  aoReincluir,
  ocupado = false,
}: {
  inventario: ResumoInventario;
  aoReincluir?: (caminho: string) => void;
  ocupado?: boolean;
}) {
  const omitidos = inventario.ignorados - inventario.itensIgnorados.length;
  return (
    <div className="resumo-inventario">
      <div role="group" aria-label="O que será analisado">
        <h3>
          Será analisado: {inventario.analisados} arquivos (
          {tamanhoLegivel(inventario.bytesAnalisados)})
        </h3>
        <table>
          <tbody>
            {inventario.porLinguagem.map((c) => (
              <tr key={c.chave}>
                <td>{c.rotulo}</td>
                <td className="numero">{c.quantidade}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div role="group" aria-label="O que fica fora">
        <h3>Fica fora: {inventario.ignorados} itens</h3>
        {inventario.porMotivo.length > 0 && (
          <table>
            <tbody>
              {inventario.porMotivo.map((c) => (
                <tr key={c.chave}>
                  <td>{c.rotulo}</td>
                  <td className="numero">{c.quantidade}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {inventario.itensIgnorados.length > 0 && (
          <details>
            <summary>Ver os itens que ficam fora</summary>
            <ul className="lista-ignorados">
              {inventario.itensIgnorados.map((i) => (
                <li key={i.caminho}>
                  <code>
                    {i.caminho}
                    {i.pasta ? "/" : ""}
                  </code>{" "}
                  <span className="muted">{i.rotulo}</span>
                  {aoReincluir && i.ajustavel && (
                    <>
                      {" "}
                      <button
                        className="btn"
                        type="button"
                        disabled={ocupado}
                        aria-label={`Incluir ${i.caminho} na análise`}
                        onClick={() => aoReincluir(i.caminho)}
                      >
                        Incluir
                      </button>
                    </>
                  )}
                </li>
              ))}
            </ul>
            {omitidos > 0 && <p className="muted">E mais {omitidos} itens.</p>}
          </details>
        )}
      </div>
    </div>
  );
}
