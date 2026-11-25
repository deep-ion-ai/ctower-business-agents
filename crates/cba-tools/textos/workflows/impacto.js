export const meta = {
  name: 'cba-impacto',
  description: 'Análise de impacto de uma demanda pelo CBA, com investigação por frente e verificação independente',
  phases: [
    { title: 'Preparar', detail: 'retoma a análise e separa as frentes de investigação' },
    { title: 'Investigar', detail: 'um agente por frente' },
    { title: 'Verificar', detail: 'um cético independente por frente' },
    { title: 'Consolidar', detail: 'abrangência, riscos transversais e onde parou' },
  ],
}

// Versão provisória (#370): só precisa ser instalada e válida. As fases completas vêm
// na execução do workflow de impacto (#371; spec
// docs/specs/2026-11-25-workflows-nas-analises.md, seção 3.1).
//
// args: "A-0042" (só o id) ou { analise: "A-0042", retomada: false, limites: {...} }.
const entrada = typeof args === 'string' ? { analise: args } : args || {}
const analise = entrada.analise
if (!analise) {
  throw new Error('informe a análise nos args (por exemplo, A-0042)')
}

phase('Preparar')
const situacao = await agent(
  'Chame a ferramenta retomar_analise do servidor MCP cba com analise ' +
    analise +
    ' e devolva, em uma frase, onde a análise parou.',
  { label: 'Preparar ' + analise },
)

return { analise: analise, situacao: situacao }
