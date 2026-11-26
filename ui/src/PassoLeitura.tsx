import { useId, useState } from "react";
import { Alerta, BotaoIcone, Cartao, ItemLista, ListaItens } from "./componentes";
import { numero, plural } from "./estadoCadastro";
import { IconeDesfazer, IconeMais } from "./icones";
import {
  tamanhoLegivel,
  type GrupoIgnorado,
  type IgnoradoVista,
  type InclusaoVista,
  type PreviaProjeto,
} from "./primeiroUso";

/** Quantos grupos a lista mostra de uma vez; o resto se acha pela busca. */
const MAX_GRUPOS_VISIVEIS = 50;

function semAcento(t: string): string {
  return t.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

/** O nome do grupo na lista: o caminho, se é um item só, ou "N arquivos .png em pasta/". */
export function rotuloDoGrupo(g: GrupoIgnorado): string {
  if (g.itens === 1 && g.exemplos[0]) return g.exemplos[0] + (g.tipo === "pasta" ? "/" : "");
  const onde = g.pasta ? `${g.pasta}/` : "a raiz do projeto";
  if (g.tipo === "pasta") return `${numero(g.itens)} pastas em ${onde}`;
  const tipo = g.tipo === "sem extensão" ? "sem extensão" : `.${g.tipo}`;
  return `${numero(g.itens)} arquivos ${tipo} em ${onde}`;
}

/** O que uma reinclusão fez, em palavras. */
export function efeitoDaInclusao(v: InclusaoVista): string {
  if (v.entram > 0 && v.continuamFora === 0) {
    return `${plural(v.entram, "arquivo entra", "arquivos entram")} na análise`;
  }
  if (v.entram > 0) {
    return `${plural(v.entram, "arquivo entra", "arquivos entram")}; ${numero(v.continuamFora)} continuam fora (binários pelo conteúdo)`;
  }
  if (v.continuamFora > 0) {
    return `continua fora: ${plural(v.continuamFora, "arquivo tem", "arquivos têm")} conteúdo binário`;
  }
  return "nenhum arquivo casa com este padrão";
}

/**
 * Passo 2 do cadastro: o que a IA vai ler. A prévia do inventário vem sozinha ao entrar
 * (e de novo quando as exclusões mudam): arquivos por linguagem, o que fica fora por
 * motivo e, em destaque, o que ficou fora por parecer ter segredos. As contagens são
 * sempre em arquivos. O ajuste fino fica recolhido: o que fica fora, agrupado por pasta e
 * tipo e com busca, o efeito de cada reinclusão e as exclusões a mais.
 */
export function PassoLeitura({
  previa,
  lendo,
  falha,
  exclusoes,
  inclusoes,
  aoMudarExclusoes,
  aoIncluir,
  aoIncluirGrupo,
  aoTirarInclusao,
  aoTentarDeNovo,
}: {
  previa: PreviaProjeto | null;
  lendo: boolean;
  falha: string | null;
  exclusoes: string;
  inclusoes: string[];
  aoMudarExclusoes: (texto: string) => void;
  aoIncluir: (caminho: string) => void;
  /** Reinclui um grupo: os padrões que o cobrem. */
  aoIncluirGrupo: (padroes: string[]) => void;
  aoTirarInclusao: (padrao: string) => void;
  aoTentarDeNovo: () => void;
}) {
  const [ajustes, setAjustes] = useState(false);
  const idAjustes = useId();
  const inv = previa?.inventario;
  const segredos = inv?.itensIgnorados.filter((i) => i.motivo === "segredo" && i.ajustavel) ?? [];
  const incluiveis =
    inv?.gruposIgnorados.filter((g) => g.ajustavel && g.padroes.length > 0 && g.motivo !== "segredo") ??
    [];
  const [busca, setBusca] = useState("");
  const termos = semAcento(busca).split(/\s+/).filter(Boolean);
  const encontrados = incluiveis.filter((g) => {
    const texto = semAcento(`${g.pasta} ${g.tipo} ${g.rotulo} ${g.exemplos.join(" ")}`);
    return termos.every((t) => texto.includes(t));
  });
  const visiveis = encontrados.slice(0, MAX_GRUPOS_VISIVEIS);
  const vistas = new Map((previa?.inclusoesVistas ?? []).map((v) => [v.padrao, v]));
  const maiorLinguagem = Math.max(1, ...(inv?.porLinguagem.map((l) => l.quantidade) ?? [1]));

  return (
    <>
      <div role="status" aria-live="polite" className="status-previa">
        {lendo && (
          <span>
            <progress aria-label="Lendo as pastas do projeto" /> Lendo as pastas do projeto…
          </span>
        )}
      </div>
      {falha && !lendo && (
        <Alerta
          tom="erro"
          titulo="Não deu para ler as pastas."
          acoes={
            <button type="button" className="btn btn-sm" onClick={aoTentarDeNovo}>
              Tentar de novo
            </button>
          }
        >
          {falha}
        </Alerta>
      )}
      {previa?.jaCadastrado && (
        <Alerta tom="erro" titulo={`Já existe um projeto cadastrado com o nome “${previa.nome}”.`}>
          Volte ao passo 1 e use outro nome.
        </Alerta>
      )}

      {inv && (
        <section
          className="previa-leitura"
          aria-label="Prévia do inventário"
          aria-busy={lendo || undefined}
        >
          <Cartao rotulo="O que será analisado">
            <p className="numero-destaque">
              <strong>{numero(inv.analisados)}</strong>{" "}
              <span>arquivos serão analisados · {tamanhoLegivel(inv.bytesAnalisados)}</span>
            </p>
            <ul className="barras-linguagem">
              {inv.porLinguagem.map((l) => (
                <li key={l.chave}>
                  <span>{l.rotulo}</span>
                  <span className="trilho" aria-hidden="true">
                    <span
                      className="barra"
                      style={{ width: `${Math.round((l.quantidade / maiorLinguagem) * 100)}%` }}
                    />
                  </span>
                  <span className="numero">{numero(l.quantidade)}</span>
                </li>
              ))}
            </ul>
          </Cartao>

          <Cartao rotulo="O que fica fora">
            <p className="numero-destaque">
              <strong>{numero(inv.ignorados)}</strong> <span>arquivos ficam fora</span>
            </p>
            {inv.porMotivo.length > 0 && (
              <ul className="motivos-fora">
                {inv.porMotivo.map((m) => (
                  <li key={m.chave}>
                    <span>{m.rotulo}</span>
                    <span className="numero">{numero(m.quantidade)}</span>
                  </li>
                ))}
              </ul>
            )}
            {segredos.length > 0 && (
              <div role="group" aria-label="Podem conter segredos">
                {/* Aviso fixo da prévia: "note", para não ser anunciado a cada releitura. */}
                <Alerta tom="atencao" titulo="Podem conter segredos." role="note">
                  <p>
                    <ListaDeCaminhos itens={segredos} />{" "}
                    {segredos.length === 1
                      ? "ficou fora por parecer ter senhas ou tokens. Se não tiver, você pode incluí-lo."
                      : "ficaram fora por parecerem ter senhas ou tokens. Se não tiverem, você pode incluí-los."}
                  </p>
                  <ListaItens>
                    {segredos.map((i) => (
                      <ItemIncluivel key={i.caminho} item={i} aoIncluir={aoIncluir} ocupado={lendo} />
                    ))}
                  </ListaItens>
                </Alerta>
              </div>
            )}
          </Cartao>
        </section>
      )}

      <Cartao como="div" className="ajustes-leitura">
        <button
          type="button"
          className="disclosure"
          aria-expanded={ajustes}
          aria-controls={idAjustes}
          onClick={() => setAjustes((a) => !a)}
        >
          <span>
            Ajustar o que fica fora <span className="muted">· avançado</span>
          </span>
          <span aria-hidden="true">{ajustes ? "▴" : "▾"}</span>
        </button>
        <div id={idAjustes} hidden={!ajustes} className="conteudo-ajustes">
          {incluiveis.length > 0 && (
            <div role="group" aria-label="Itens fora que você pode incluir">
              <p className="subtitulo-passo">Itens fora que você pode incluir</p>
              <div className="field">
                <label htmlFor={`${idAjustes}-busca`}>Buscar por pasta ou tipo</label>
                <input
                  id={`${idAjustes}-busca`}
                  type="search"
                  spellCheck={false}
                  placeholder="Ex.: icons, png, docs"
                  value={busca}
                  onChange={(e) => setBusca(e.target.value)}
                />
              </div>
              <div className="rolagem-incluiveis">
                <ListaItens>
                  {visiveis.map((g) => (
                    <GrupoIncluivel
                      key={`${g.pasta}|${g.tipo}|${g.motivo}`}
                      grupo={g}
                      aoIncluir={aoIncluirGrupo}
                      ocupado={lendo}
                    />
                  ))}
                </ListaItens>
              </div>
              <p className="muted" role="status">
                {encontrados.length === 0
                  ? "Nada encontrado com essa busca."
                  : encontrados.length > visiveis.length
                    ? `Mostrando ${numero(visiveis.length)} de ${numero(encontrados.length)} grupos. Use a busca para achar os outros.`
                    : `${plural(encontrados.length, "grupo", "grupos")} (por pasta e tipo).`}
              </p>
            </div>
          )}
          {inclusoes.length > 0 && (
            <div role="group" aria-label="Reincluídos">
              <p className="subtitulo-passo">Incluídos por você (ficam visíveis para a IA)</p>
              <ListaItens>
                {inclusoes.map((x) => (
                  <ItemLista
                    key={x}
                    acoes={
                      <BotaoIcone
                        rotulo={`Tirar a reinclusão de ${x}`}
                        dica="Desfazer a reinclusão"
                        onClick={() => aoTirarInclusao(x)}
                      >
                        <IconeDesfazer />
                      </BotaoIcone>
                    }
                  >
                    <code>{x}</code>
                    {vistas.get(x) && (
                      <>
                        {" "}
                        <span className="muted">· {efeitoDaInclusao(vistas.get(x)!)}</span>
                      </>
                    )}
                  </ItemLista>
                ))}
              </ListaItens>
            </div>
          )}
          <div className="field">
            <label htmlFor={`${idAjustes}-exclusoes`}>
              Excluir também (um padrão por linha, como no .gitignore: <code>docs/antigos/</code>,{" "}
              <code>*.log</code>)
            </label>
            <textarea
              id={`${idAjustes}-exclusoes`}
              rows={3}
              spellCheck={false}
              value={exclusoes}
              onChange={(e) => aoMudarExclusoes(e.target.value)}
            />
          </div>
        </div>
      </Cartao>
    </>
  );
}

function ListaDeCaminhos({ itens }: { itens: IgnoradoVista[] }) {
  const primeiros = itens.slice(0, 2);
  const resto = itens.length - primeiros.length;
  return (
    <>
      {primeiros.map((i, k) => (
        <span key={i.caminho}>
          {k > 0 && (resto > 0 ? ", " : " e ")}
          <code>{i.caminho}</code>
        </span>
      ))}
      {resto > 0 && ` e mais ${resto}`}
    </>
  );
}

function ItemIncluivel({
  item,
  aoIncluir,
  ocupado,
}: {
  item: IgnoradoVista;
  aoIncluir: (caminho: string) => void;
  ocupado: boolean;
}) {
  return (
    <ItemLista
      acoes={
        <BotaoIcone
          rotulo={`Incluir ${item.caminho} na análise`}
          dica="Incluir na análise"
          disabled={ocupado}
          onClick={() => aoIncluir(item.caminho)}
        >
          <IconeMais />
        </BotaoIcone>
      }
    >
      <code>
        {item.caminho}
        {item.pasta ? "/" : ""}
      </code>{" "}
      <span className="muted">· {item.rotulo}</span>
    </ItemLista>
  );
}

function GrupoIncluivel({
  grupo,
  aoIncluir,
  ocupado,
}: {
  grupo: GrupoIgnorado;
  aoIncluir: (padroes: string[]) => void;
  ocupado: boolean;
}) {
  const nome = rotuloDoGrupo(grupo);
  return (
    <ItemLista
      acoes={
        <BotaoIcone
          rotulo={`Incluir ${nome} na análise`}
          dica="Incluir na análise"
          disabled={ocupado}
          onClick={() => aoIncluir(grupo.padroes)}
        >
          <IconeMais />
        </BotaoIcone>
      }
    >
      <code>{nome}</code>{" "}
      <span className="muted">
        · {grupo.rotulo} · {plural(grupo.arquivos, "arquivo", "arquivos")}
      </span>
    </ItemLista>
  );
}
