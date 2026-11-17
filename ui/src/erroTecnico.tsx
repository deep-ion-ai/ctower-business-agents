/**
 * Mensagens técnicas (git, parser) chegam em inglês. Aqui viram uma frase em português
 * e a mensagem original fica recolhida em "Detalhe técnico".
 */

const REGRAS: { padrao: RegExp; texto: string }[] = [
  {
    padrao: /repository .*not found|does not appear to be a git repository/i,
    texto:
      "Repositório não encontrado. Confira o endereço e se o token tem acesso a ele.",
  },
  {
    padrao: /authentication failed|could not read username|invalid username or password|permission denied|http basic: access denied|returned error: 40[13]/i,
    texto: "O acesso foi recusado. Confira o token e as permissões dele no repositório.",
  },
  {
    padrao: /could not resolve host|unable to access|connection (timed out|refused)|network is unreachable/i,
    texto: "Não foi possível alcançar o servidor. Confira a conexão e o endereço.",
  },
  {
    padrao: /found unexpected end of stream|did not find expected|mapping values are not allowed|while parsing|expected .* but found/i,
    texto:
      "O arquivo está com a estrutura incompleta ou mal formada e não pôde ser lido. Confira o cabeçalho (entre ---) do documento.",
  },
];

/** A frase em português para uma mensagem técnica conhecida, ou `null` se não for uma. */
export function traduzirErro(mensagem: string): string | null {
  return REGRAS.find((r) => r.padrao.test(mensagem))?.texto ?? null;
}

/** Mostra o erro; se for técnico e conhecido, traduzido, com o original em `<details>`. */
export function ErroTecnico({ mensagem }: { mensagem: string }) {
  const traduzida = traduzirErro(mensagem);
  if (!traduzida) return <>{mensagem}</>;
  return (
    <>
      {traduzida}
      <details className="detalhe-tecnico">
        <summary>Detalhe técnico</summary>
        <pre className="mono">{mensagem}</pre>
      </details>
    </>
  );
}
