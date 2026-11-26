import { useEffect, useMemo, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, BarraAcoes, EstadoVazio, Secao, Selo } from "./componentes";
import { problemaDaUrl, type SituacaoAmbienteTeste } from "./ambienteTeste";
import type { PropsConfigProjeto } from "./ConfigProjeto";
import { DialogoConfirmacao } from "./DialogoConfirmacao";

/** Linha da lista de usuários na tela. */
interface Linha {
  /** Chave local (React), estável enquanto a tela está aberta. */
  chave: number;
  /** `null` até salvar. */
  id: string | null;
  usuario: string;
  perfil: string;
  descricao: string;
  /** Senha digitada agora; vazia mantém a guardada. */
  senha: string;
  temSenha: boolean;
}

interface Formulario {
  url: string;
  observacoes: string;
  linhas: Linha[];
}

let proximaChave = 1;

function formularioDe(s: SituacaoAmbienteTeste | null): Formulario {
  const a = s?.ambiente;
  return {
    url: a?.url ?? "",
    observacoes: a?.observacoes ?? "",
    linhas: (a?.usuarios ?? []).map((u) => ({
      chave: proximaChave++,
      id: u.id,
      usuario: u.usuario,
      perfil: u.perfil,
      descricao: u.descricao ?? "",
      senha: "",
      temSenha: s?.comSenha.includes(u.id) ?? false,
    })),
  };
}

function linhaNova(): Linha {
  return {
    chave: proximaChave++,
    id: null,
    usuario: "",
    perfil: "",
    descricao: "",
    senha: "",
    temSenha: false,
  };
}

/** Compara só o que é gravado (a senha digitada conta como mudança). */
function assinatura(f: Formulario): string {
  return JSON.stringify([
    f.url.trim(),
    f.observacoes.trim(),
    f.linhas.map((l) => [l.id, l.usuario.trim(), l.perfil.trim(), l.descricao.trim(), l.senha]),
  ]);
}

/**
 * Aba “Ambiente de teste”: URL da aplicação em teste e os usuários de teste com o
 * perfil de cada um, para simular o uso e não deixar a análise limitada ao código.
 * Nesta etapa, só aplicações web com login por usuário e senha. A senha vai para o
 * chaveiro do sistema e nunca volta para a tela.
 */
export function AbaAmbienteTeste({ api, projeto, aoPendencia }: PropsConfigProjeto) {
  const [situacao, setSituacao] = useState<SituacaoAmbienteTeste | null | undefined>(undefined);
  const [form, setForm] = useState<Formulario>(() => formularioDe(null));
  const [salvo, setSalvo] = useState<string>(() => assinatura(formularioDe(null)));
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [tentou, setTentou] = useState(false);
  const [remover, setRemover] = useState(false);
  const pendencia = useRef(aoPendencia);
  pendencia.current = aoPendencia;

  const aplicar = (s: SituacaoAmbienteTeste) => {
    const f = formularioDe(s);
    setSituacao(s);
    setForm(f);
    setSalvo(assinatura(f));
    setTentou(false);
  };

  useEffect(() => {
    let vivo = true;
    api
      .ambienteTeste(projeto.id)
      .then((s) => vivo && aplicar(s))
      .catch((e: unknown) => {
        if (!vivo) return;
        setSituacao(null);
        setFalha(`Não foi possível ler o ambiente de teste: ${mensagemDe(e)}`);
      });
    return () => {
      vivo = false;
    };
  }, [api, projeto.id]);

  const pendente = situacao !== undefined && assinatura(form) !== salvo;
  useEffect(() => {
    pendencia.current?.(pendente);
  }, [pendente]);

  const problemas = useMemo(() => {
    const lista: string[] = [];
    const url = problemaDaUrl(form.url);
    if (url) lista.push(url);
    const vistos = new Set<string>();
    form.linhas.forEach((l, i) => {
      const nome = l.usuario.trim() || `da linha ${i + 1}`;
      if (!l.usuario.trim()) lista.push(`Informe o usuário da linha ${i + 1}.`);
      if (!l.perfil.trim()) lista.push(`Informe o perfil do usuário ${nome}.`);
      const chave = l.usuario.trim().toLowerCase();
      if (chave && vistos.has(chave)) lista.push(`O usuário ${nome} aparece mais de uma vez.`);
      vistos.add(chave);
    });
    return lista;
  }, [form]);

  const mudarLinha = (chave: number, campo: Partial<Linha>) =>
    setForm((f) => ({
      ...f,
      linhas: f.linhas.map((l) => (l.chave === chave ? { ...l, ...campo } : l)),
    }));

  const salvar = () => {
    setTentou(true);
    setAviso(null);
    if (problemas.length > 0) return;
    setOcupado(true);
    setFalha(null);
    api
      .salvarAmbienteTeste(projeto.id, {
        url: form.url,
        observacoes: form.observacoes,
        usuarios: form.linhas.map((l) => ({
          id: l.id,
          usuario: l.usuario,
          perfil: l.perfil,
          descricao: l.descricao,
          senha: l.senha || null,
        })),
      })
      .then((s) => {
        aplicar(s);
        setAviso("Ambiente de teste salvo.");
      })
      .catch((e: unknown) => setFalha(`Não foi possível salvar: ${mensagemDe(e)}`))
      .finally(() => setOcupado(false));
  };

  const confirmarRemocao = () => {
    setOcupado(true);
    setFalha(null);
    api
      .removerAmbienteTeste(projeto.id)
      .then((s) => {
        aplicar(s);
        setRemover(false);
        setAviso("Ambiente de teste removido. As senhas foram apagadas do chaveiro.");
      })
      .catch((e: unknown) => setFalha(`Não foi possível remover: ${mensagemDe(e)}`))
      .finally(() => setOcupado(false));
  };

  if (situacao === undefined) {
    return <p className="muted">Carregando o ambiente de teste…</p>;
  }

  const semSenha = form.linhas.filter((l) => !l.temSenha && !l.senha).length;
  const perfis = new Set(form.linhas.map((l) => l.perfil.trim().toLowerCase()).filter(Boolean));

  return (
    <form
      className="aba-ambiente-teste"
      aria-label="Ambiente de teste"
      onSubmit={(e) => {
        e.preventDefault();
        salvar();
      }}
    >
      <p className="muted">
        Onde a aplicação roda para testes e com quais usuários entrar, um por perfil de acesso.
        Serve para simular o uso e não deixar a análise limitada ao código. Por enquanto, só
        aplicações web com login por usuário e senha.
      </p>

      <Secao
        titulo="Aplicação"
        nivel={4}
        rotulo="Aplicação em teste"
        acoes={<Selo>Web · usuário e senha</Selo>}
      >
        <div className="ambiente-teste-campos">
          <label className="field">
            URL do ambiente de teste
            <input
              type="text"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
              placeholder="https://homologacao.exemplo.com.br/login"
              value={form.url}
              aria-invalid={tentou && problemaDaUrl(form.url) !== null}
              onChange={(e) => setForm({ ...form, url: e.target.value })}
            />
          </label>
          <label className="field">
            Observações (opcional)
            <textarea
              rows={2}
              placeholder="Ex.: precisa de VPN; base restaurada toda segunda-feira."
              value={form.observacoes}
              onChange={(e) => setForm({ ...form, observacoes: e.target.value })}
            />
          </label>
        </div>
      </Secao>

      <Secao
        titulo="Usuários de teste"
        nivel={4}
        acoes={
          form.linhas.length > 0 && (
            <Selo>
              {form.linhas.length} {form.linhas.length === 1 ? "usuário" : "usuários"} ·{" "}
              {perfis.size} {perfis.size === 1 ? "perfil" : "perfis"}
            </Selo>
          )
        }
      >
        {form.linhas.length === 0 ? (
          <EstadoVazio>
            Nenhum usuário cadastrado. Cadastre um usuário para cada perfil que a análise precisa
            simular (por exemplo, atendente, gestor e auditor).
          </EstadoVazio>
        ) : (
          <ul className="lista-config lista-usuarios-teste">
            {form.linhas.map((l, i) => {
              const nome = l.usuario.trim() || `usuário ${i + 1}`;
              return (
                <li key={l.chave} className="linha-config usuario-teste">
                  <div className="field-pair">
                    <label className="field">
                      Usuário (login)
                      <input
                        type="text"
                        autoComplete="off"
                        spellCheck={false}
                        value={l.usuario}
                        aria-invalid={tentou && !l.usuario.trim()}
                        onChange={(e) => mudarLinha(l.chave, { usuario: e.target.value })}
                      />
                    </label>
                    <label className="field">
                      Senha
                      <input
                        type="password"
                        autoComplete="new-password"
                        spellCheck={false}
                        placeholder={l.temSenha ? "guardada no chaveiro" : ""}
                        value={l.senha}
                        onChange={(e) => mudarLinha(l.chave, { senha: e.target.value })}
                      />
                    </label>
                    <label className="field">
                      Perfil
                      <input
                        type="text"
                        autoComplete="off"
                        placeholder="Ex.: Atendente"
                        value={l.perfil}
                        aria-invalid={tentou && !l.perfil.trim()}
                        onChange={(e) => mudarLinha(l.chave, { perfil: e.target.value })}
                      />
                    </label>
                  </div>
                  <label className="field">
                    O que o perfil pode fazer (opcional)
                    <input
                      type="text"
                      autoComplete="off"
                      placeholder="Ex.: abre e consulta faturas; não aprova parcelamento."
                      value={l.descricao}
                      onChange={(e) => mudarLinha(l.chave, { descricao: e.target.value })}
                    />
                  </label>
                  <div className="usuario-teste-rodape">
                    {l.senha ? (
                      <span className="muted">A senha nova será guardada ao salvar.</span>
                    ) : l.temSenha ? (
                      <span className="ok-text">✓ Senha guardada no chaveiro</span>
                    ) : (
                      <span className="attention-text">Sem senha guardada</span>
                    )}
                    <button
                      className="btn btn-sm"
                      type="button"
                      aria-label={`Remover ${nome}`}
                      onClick={() =>
                        setForm((f) => ({
                          ...f,
                          linhas: f.linhas.filter((x) => x.chave !== l.chave),
                        }))
                      }
                    >
                      Remover
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
        <BarraAcoes
          rotulo="Ações dos usuários de teste"
          secundarias={[
            {
              texto: "+ Adicionar usuário",
              aoEscolher: () => setForm((f) => ({ ...f, linhas: [...f.linhas, linhaNova()] })),
            },
          ]}
        />
      </Secao>

      <p className="muted aviso-senhas">
        As senhas ficam no chaveiro do sistema (Keychain no macOS, Gerenciador de Credenciais no
        Windows), nunca no cadastro, na base de conhecimento nem nos logs. Use só usuários de
        teste, nunca de produção.
      </p>

      {tentou && problemas.length > 0 && (
        <Alerta tom="erro" titulo="Falta acertar antes de salvar:">
          <ul>
            {problemas.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        </Alerta>
      )}
      {falha && <Alerta tom="erro" titulo={falha} />}
      {aviso && !pendente && (
        <p className="ok-text" role="status">
          {aviso}
          {semSenha > 0 &&
            ` ${semSenha} ${semSenha === 1 ? "usuário está" : "usuários estão"} sem senha.`}
        </p>
      )}

      <BarraAcoes rotulo="Ações do ambiente de teste">
        <button className="btn btn-primary" type="submit" disabled={ocupado || !pendente}>
          {ocupado ? "Salvando…" : "Salvar"}
        </button>
        {pendente && situacao !== null && (
          <button
            className="btn"
            type="button"
            disabled={ocupado}
            onClick={() => {
              const f = formularioDe(situacao);
              setForm(f);
              setTentou(false);
            }}
          >
            Desfazer as alterações
          </button>
        )}
        {situacao?.ambiente && (
          <button
            className="btn btn-danger"
            type="button"
            disabled={ocupado}
            onClick={() => setRemover(true)}
          >
            Remover o ambiente…
          </button>
        )}
      </BarraAcoes>

      {remover && (
        <DialogoConfirmacao
          titulo="Remover o ambiente de teste?"
          aoCancelar={() => !ocupado && setRemover(false)}
          acoes={
            <>
              <button
                className="btn"
                type="button"
                autoFocus
                disabled={ocupado}
                onClick={() => setRemover(false)}
              >
                Cancelar
              </button>
              <button
                className="btn btn-danger"
                type="button"
                disabled={ocupado}
                onClick={confirmarRemocao}
              >
                Remover o ambiente
              </button>
            </>
          }
        >
          <p>
            A URL, os usuários e os perfis saem do projeto, e as senhas são apagadas do chaveiro
            do sistema. Não dá para desfazer.
          </p>
        </DialogoConfirmacao>
      )}
    </form>
  );
}
