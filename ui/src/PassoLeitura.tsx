import { useId, useState } from "react";
import { numero, plural } from "./estadoCadastro";
import { tamanhoLegivel, type IgnoradoVista, type PreviaProjeto } from "./primeiroUso";

/**
 * Passo 2 do cadastro: o que a IA vai ler. A prévia do inventário vem sozinha ao entrar
 * (e de novo quando as exclusões mudam): arquivos por linguagem, o que fica fora por
 * motivo e, em destaque, o que ficou fora por parecer ter segredos. O ajuste fino das
 * exclusões fica recolhido.
 */
export function PassoLeitura({
  previa,
  lendo,
  falha,
  exclusoes,
  inclusoes,
  aoMudarExclusoes,
  aoIncluir,
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
  aoTirarInclusao: (padrao: string) => void;
  aoTentarDeNovo: () => void;
}) {
  const [ajustes, setAjustes] = useState(false);
  const idAjustes = useId();
  const inv = previa?.inventario;
  const segredos = inv?.itensIgnorados.filter((i) => i.motivo === "segredo" && i.ajustavel) ?? [];
  const incluiveis = inv?.itensIgnorados.filter((i) => i.ajustavel && i.motivo !== "segredo") ?? [];
  const omitidos = inv ? inv.ignorados - inv.itensIgnorados.length : 0;
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
        <div className="falha-previa">
          <p className="error-text" role="alert">
            {falha}
          </p>
          <button type="button" className="btn btn-sm" onClick={aoTentarDeNovo}>
            Tentar de novo
          </button>
        </div>
      )}
      {previa?.jaCadastrado && (
        <p className="error-text" role="alert">
          Já existe um projeto cadastrado com o nome “{previa.nome}”. Volte ao passo 1 e use outro
          nome.
        </p>
      )}

      {inv && (
        <section
          className="previa-leitura"
          aria-label="Prévia do inventário"
          aria-busy={lendo || undefined}
        >
          <div className="cartao-assistente" role="group" aria-label="O que será analisado">
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
          </div>

          <div className="cartao-assistente" role="group" aria-label="O que fica fora">
            <p className="numero-destaque">
              <strong>{numero(inv.ignorados)}</strong> <span>itens ficam fora</span>
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
              <div className="aviso-segredos" role="group" aria-label="Podem conter segredos">
                <p>
                  <ListaDeCaminhos itens={segredos} />{" "}
                  {segredos.length === 1
                    ? "ficou fora por parecer ter senhas ou tokens. Se não tiver, você pode incluí-lo."
                    : "ficaram fora por parecerem ter senhas ou tokens. Se não tiverem, você pode incluí-los."}
                </p>
                <ul>
                  {segredos.map((i) => (
                    <ItemIncluivel key={i.caminho} item={i} aoIncluir={aoIncluir} ocupado={lendo} />
                  ))}
                </ul>
              </div>
            )}
          </div>
        </section>
      )}

      <div className="cartao-assistente ajustes-leitura">
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
              <ul className="lista-incluiveis">
                {incluiveis.map((i) => (
                  <ItemIncluivel key={i.caminho} item={i} aoIncluir={aoIncluir} ocupado={lendo} />
                ))}
              </ul>
              {omitidos > 0 && <p className="muted">E mais {plural(omitidos, "item", "itens")}.</p>}
            </div>
          )}
          {inclusoes.length > 0 && (
            <div role="group" aria-label="Reincluídos">
              <p className="subtitulo-passo">Incluídos por você (ficam visíveis para a IA)</p>
              <ul className="lista-incluiveis">
                {inclusoes.map((x) => (
                  <li key={x}>
                    <code>{x}</code>
                    <button
                      className="btn btn-sm"
                      type="button"
                      aria-label={`Tirar a reinclusão de ${x}`}
                      onClick={() => aoTirarInclusao(x)}
                    >
                      Desfazer
                    </button>
                  </li>
                ))}
              </ul>
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
      </div>
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
    <li>
      <span>
        <code>
          {item.caminho}
          {item.pasta ? "/" : ""}
        </code>{" "}
        <span className="muted">· {item.rotulo}</span>
      </span>
      <button
        className="btn btn-sm"
        type="button"
        disabled={ocupado}
        aria-label={`Incluir ${item.caminho} na análise`}
        onClick={() => aoIncluir(item.caminho)}
      >
        Incluir
      </button>
    </li>
  );
}
