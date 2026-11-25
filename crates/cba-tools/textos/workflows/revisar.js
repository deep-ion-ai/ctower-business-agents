export const meta = {
  name: 'cba-revisar',
  description: 'Revisão de um requisito ou história pelo CBA, com avaliação por critério de aceite e verificação independente dos achados',
  whenToUse: 'Só quando o pedido do analista manda chamar o workflow cba-revisar para uma análise A-NNNN do ctower.',
  phases: [
    { title: 'Separar', detail: 'retoma a análise e separa os critérios de aceite e as afirmações do texto' },
    { title: 'Avaliar', detail: 'um agente por critério' },
    { title: 'Verificar', detail: 'um cético independente por critério' },
    { title: 'Consolidar', detail: 'completude do texto inteiro, INVEST e veredito' },
  ],
}

// Workflow padrão da revisão de requisito do CBA (spec
// docs/specs/2026-11-25-workflows-nas-analises.md, seção 3.1; #373).
//
// args: "A-0042" (só o id), "A-0042 retomada" (conversa nova sobre uma revisão já
// começada) ou { analise: "A-0042", retomada: false, limites: { frentes: 6,
// esforco: "medium", esforcoVerificacao: "low" } }. Aqui, as frentes são os
// critérios de aceite e as afirmações do texto. Os limites ausentes usam os padrões
// abaixo. O modelo dos agentes não é fixado: eles herdam o da sessão.
//
// Os agentes registram direto no parecer pelas ferramentas do servidor MCP cba, as
// mesmas da condução simples. O workflow não conclui: a sessão principal chama
// concluir_analise e responde no chat.
function lerTexto(texto) {
  const partes = texto.trim().split(/\s+/)
  return { analise: partes[0], retomada: partes.indexOf('retomada') > 0 }
}

const entrada = typeof args === 'string' ? lerTexto(args) : args || {}
const analise = String(entrada.analise || '').trim().toUpperCase()
if (!/^A-\d{4,}$/.test(analise)) {
  throw new Error('informe a análise nos args, como A-0042 (recebido: ' + JSON.stringify(args) + ')')
}
const retomada = entrada.retomada === true
const limites = entrada.limites || {}
const criteriosNoMaximo = Math.max(1, Math.min(12, Number(limites.frentes) || 6))
const esforco = limites.esforco || 'medium'
const esforcoVerificacao = limites.esforcoVerificacao || 'low'

const COMUM =
  'Você é um agente do workflow cba-revisar, na análise ' + analise + ' do ctower (revisão de um requisito ou história). ' +
  'As ferramentas do servidor MCP cba (mcp__cba__...) carregam-se com ToolSearch; passe sempre analise ' + analise + '. ' +
  'Você não altera arquivos do projeto nem o texto do autor: só lê o código (Read, Grep, Glob e as ferramentas de consulta do cba: mapa, estrutura, referencias, schema, conhecimento). ' +
  'Não chame iniciar_analise nem concluir_analise, e não siga o roteiro que retomar_analise devolve: siga só este pedido. ' +
  'Escreva para quem não lê código, em português.'

const REGRAS_DE_ACHADO =
  'O diferencial da revisão é conferir o texto contra a base e o código: um texto bem escrito que contradiz o que o sistema faz é o achado mais importante. ' +
  'Consulte a base antes do código (conhecimento, um conceito por chamada): regra confirmada (RN-...) não precisa ser redescoberta; confira se a evidência dela ainda vale. ' +
  'Registre cada problema com registrar_achado, um achado por problema, com a característica da ISO/IEC/IEEE 29148: ' +
  'correcao quando o texto contradiz uma regra confirmada ou o que o código faz (diga no problema o que a regra ou o código fazem, com valores; cite em regras a RN contrariada e em evidencias o trecho do código; sem evidência é recusado); ' +
  'ambiguidade para termo vago (um achado por termo, ou termos da mesma frase juntos, com a reescrita verificável); ' +
  'verificabilidade para critério que não se testa com resultado objetivo; ' +
  'completude para exceção, perfil, limite ou valor que o código trata e o texto não diz (o texto que omite uma condição não a contradiz: é completude, não correção; o trecho pode ficar vazio, cite a evidência); ' +
  'consistencia para termo que diverge do glossário; singularidade quando o trecho junta mais de uma necessidade. ' +
  'Severidade: alta quando o texto, como está, leva a construir a coisa errada; media quando gera dúvida ou retrabalho provável; baixa para redação. ' +
  'O trecho é copiado do texto revisado, como está; trecho que não aparece no texto é recusado. Cada evidência é o arquivo relativo à raiz do projeto, as linhas e o trecho copiado do código exatamente como está; evidência recusada volta com o motivo: corrija e chame de novo. Documentação não é evidência: o código é. ' +
  'Na dúvida se a diferença é erro do texto ou mudança pedida de propósito, registre o achado e pergunte ao autor com perguntar, escrevendo para quem escreveu o texto. ' +
  'Fatos do sistema que sustentam os achados podem ir em afirmar, secao contexto_no_codigo, uma regra por chamada, com evidência e a confiança honesta. ' +
  'Antes de registrar, confira com retomar_analise se outro agente já registrou o mesmo achado ou a mesma pergunta: não registre de novo.'

const CRITERIOS = {
  type: 'object',
  properties: {
    texto: { type: 'string', description: 'O texto a revisar, como o autor escreveu (a entrada da análise), sem resumir.' },
    historia: { type: 'boolean', description: 'O texto é uma história de usuário ("Como <perfil>, quero <ação>, para <valor>").' },
    criterios: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          titulo: { type: 'string', description: 'Título curto e único do critério ou da afirmação.' },
          trecho: { type: 'string', description: 'O trecho do texto revisado, copiado como está.' },
          foco: { type: 'string', description: 'O que conferir: as regras, valores, perfis e condições que o trecho afirma ou pede.' },
          termos_vagos: { type: 'array', items: { type: 'string' }, description: 'Os termos vagos de verificar_termos_vagos que estão neste trecho.' },
          pistas: { type: 'array', items: { type: 'string' }, description: 'Regras da base, arquivos, rotinas, tabelas ou telas por onde começar.' },
        },
        required: ['titulo', 'trecho', 'foco'],
      },
    },
    ja_avaliados: { type: 'array', items: { type: 'string' }, description: 'Na retomada: os critérios que a memória já tem como avaliados.' },
  },
  required: ['texto', 'historia', 'criterios'],
}

const AVALIADO = {
  type: 'object',
  properties: {
    achados: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          numero: { type: 'integer' },
          caracteristica: { type: 'string' },
          severidade: { type: 'string' },
          trecho: { type: 'string' },
          problema: { type: 'string' },
          regras: { type: 'array', items: { type: 'string' } },
          evidencias: { type: 'array', items: { type: 'string' }, description: 'arquivo:linha_inicial-linha_final de cada evidência.' },
        },
        required: ['numero', 'caracteristica', 'problema'],
      },
      description: 'Os achados que você registrou neste critério (o número vem da resposta de registrar_achado).',
    },
    perguntas: { type: 'integer', description: 'Quantas perguntas você registrou.' },
    resumo: { type: 'string', description: 'O que o critério mostrou, em até três frases.' },
  },
  required: ['achados', 'resumo'],
}

const VERIFICADO = {
  type: 'object',
  properties: {
    verificados: { type: 'integer' },
    contestados: {
      type: 'array',
      items: {
        type: 'object',
        properties: { numero: { type: 'integer' }, motivo: { type: 'string' } },
        required: ['numero', 'motivo'],
      },
    },
  },
  required: ['verificados', 'contestados'],
}

phase('Separar')
const separacao = await agent(
  COMUM + '\n\n' +
    'Fase Separar. Chame retomar_analise para ler o texto a revisar (a entrada), o que já foi registrado e a memória da análise, e verificar_termos_vagos para os termos vagos da lista de referência. ' +
    'Diga se o texto é uma história de usuário. Separe em até ' + criteriosNoMaximo + ' itens a avaliar, cada um com o trecho copiado como está: cada critério de aceite e cada afirmação do texto (a narrativa "Como... quero... para...", as regras, os valores, os perfis e as condições que o texto afirma). ' +
    'Junte num item as afirmações curtas sobre o mesmo assunto, e distribua cada termo vago no item cujo trecho o contém. Menos itens é melhor quando o texto é curto: não crie item sem o que conferir. ' +
    'Indique as pistas por onde começar (regras da base e funcionalidades com conhecimento e mapa), sem investigar a fundo. ' +
    (retomada
      ? 'Esta é uma retomada: a seção investigado da memória diz quais critérios já foram avaliados (itens "Critério <título>: ..."). Devolva em criterios só os que ainda faltam, com os mesmos títulos, e os já avaliados em ja_avaliados. Não grave os critérios de novo na memória. '
      : 'Grave os itens na memória com memorizar, secao contexto, um item: "Critérios do workflow: 1. <título>: <trecho>; 2. ...". ') +
    'Não registre achados, perguntas nem afirmações nesta fase.',
  { label: 'Separar ' + analise, schema: CRITERIOS, effort: esforco },
)
if (!separacao) {
  throw new Error('a fase Separar não devolveu os critérios')
}

let criterios = (separacao.criterios || []).filter(function (c) { return c && c.titulo })
if (criterios.length > criteriosNoMaximo) {
  log('Separar devolveu ' + criterios.length + ' critérios; ficam os ' + criteriosNoMaximo + ' primeiros (limites.frentes). A fase Consolidar confere o texto inteiro.')
  criterios = criterios.slice(0, criteriosNoMaximo)
}
if (retomada && (separacao.ja_avaliados || []).length > 0) {
  log('Retomada: ' + separacao.ja_avaliados.length + ' critério(s) já avaliado(s) não são refeitos.')
}

const resultados = await pipeline(
  criterios,
  function (_, criterio) {
    return agent(
      COMUM + '\n\n' +
        'Fase Avaliar, critério "' + criterio.titulo + '". Outros agentes avaliam os outros critérios em paralelo; fique no seu.\n\n' +
        'Texto revisado, como o autor escreveu:\n' + separacao.texto + '\n\n' +
        'Trecho deste critério:\n' + criterio.trecho + '\n\n' +
        'O que conferir: ' + criterio.foco + '\n' +
        (criterio.termos_vagos && criterio.termos_vagos.length ? 'Termos vagos deste trecho (um achado de ambiguidade ou verificabilidade para cada): ' + criterio.termos_vagos.join(', ') + '\n' : '') +
        (criterio.pistas && criterio.pistas.length ? 'Por onde começar: ' + criterio.pistas.join('; ') + '\n' : '') +
        '\nConfira no código, uma por uma, cada regra, valor, perfil e condição que o trecho afirma, inclusive nas outras camadas (tela, serviço, banco, lotes), e compare com as regras e exceções que o código aplica ao assunto do trecho. Avalie também se o trecho é verificável e se é singular. ' +
        'Não registre INVEST nem veredito: isso é da fase Consolidar.\n\n' +
        REGRAS_DE_ACHADO + '\n\n' +
        'Ao terminar, grave com memorizar, secao investigado, um item: "Critério ' + criterio.titulo + ': <o que concluiu, com as regras e os arquivos principais>". ' +
        'Devolva os achados que você registrou, com o número que registrar_achado devolveu.',
      { label: 'Avaliar: ' + criterio.titulo, phase: 'Avaliar', schema: AVALIADO, effort: esforco },
    )
  },
  function (avaliado, criterio) {
    if (!avaliado) {
      return { achados: 0, verificacao: null, semResultado: true }
    }
    const achados = avaliado.achados || []
    if (achados.length === 0) {
      return { achados: 0, verificacao: null }
    }
    const lista = achados
      .map(function (a) {
        return a.numero + ' [' + a.caracteristica + (a.severidade ? ', ' + a.severidade : '') + '] ' +
          (a.trecho ? 'Trecho: "' + a.trecho + '". ' : '') + a.problema +
          (a.regras && a.regras.length ? ' Regras: ' + a.regras.join(', ') + '.' : '') +
          (a.evidencias && a.evidencias.length ? ' Evidências: ' + a.evidencias.join(', ') : '')
      })
      .join('\n')
    return agent(
      COMUM + '\n\n' +
        'Fase Verificar, critério "' + criterio.titulo + '". Você é um verificador cético e independente: não foi você quem registrou estes achados. ' +
        'Texto revisado:\n' + separacao.texto + '\n\n' +
        'Para cada achado, releia o texto, a regra da base citada e o código da evidência (Read nas linhas e em volta delas) e tente refutá-lo: o texto diz mesmo isso? a regra e o código fazem mesmo o que o achado diz, com esses valores e condições? outra camada contradiz? é contradição (correcao) ou só omissão (completude)? a severidade é honesta? ' +
        'Quando o achado não se sustenta, chame contestar_achado com o número, o motivo em linguagem de negócio, com valores, e, se achar, a evidência do contrário. ' +
        'Achado que se sustenta não precisa de chamada. Não registre achados nem perguntas, e não conteste por estilo ou redação.\n\n' +
        'Achados do critério:\n' + lista + '\n\n' +
        'Devolva quantos verificou e os que contestou.',
      { label: 'Verificar: ' + criterio.titulo, phase: 'Verificar', schema: VERIFICADO, effort: esforcoVerificacao },
    ).then(function (v) {
      return { achados: achados.length, verificacao: v }
    })
  },
)

const feitos = resultados.filter(Boolean)
const achados = feitos.reduce(function (n, r) { return n + (r.achados || 0) }, 0)
const contestados = feitos.reduce(function (n, r) {
  return n + ((r.verificacao && r.verificacao.contestados) || []).length
}, 0)
const falharam = resultados.filter(function (r) { return !r || r.semResultado }).length
if (falharam > 0) {
  log(falharam + ' critério(s) sem resultado (agente com erro ou pulado); a sessão completa o que faltar.')
}

phase('Consolidar')
const consolidado = await agent(
  COMUM + '\n\n' +
    'Fase Consolidar. Os critérios foram avaliados e verificados por outros agentes. Chame retomar_analise para ver o parecer (achados, perguntas, afirmações) e verificar_termos_vagos para os termos sem achado. ' +
    'Os achados a revalidar, contestados pela verificação independente, ficam para o analista decidir: não os registre de novo e não se apoie neles para o veredito. ' +
    'Faça só o que falta, sem repetir o que já está registrado:\n' +
    '1. O texto inteiro: casos de exceção, perfis, limites e aprovações que o código aplica ao assunto e nenhum critério cobre (completude, com evidência), e um texto que junta mais de uma necessidade (singularidade). Se o mesmo problema já tem achado, não registre outro.\n' +
    '2. Termo vago da lista ainda sem achado: registre o achado de ambiguidade ou verificabilidade.\n' +
    (separacao.historia
      ? '3. O texto é uma história: faça a verificação INVEST com registrar_invest, os seis critérios (independente, negociavel, valiosa, estimavel, pequena, testavel), cada um com o resultado e uma justificativa curta, coerente com os achados.\n'
      : '3. O texto não é uma história de usuário: não registre INVEST.\n') +
    '4. Registre o veredito com registrar_veredito: pronta, pronta_com_ressalvas ou precisa_de_retrabalho, com uma justificativa curta. Achado de severidade alta que se sustenta pede retrabalho; os demais, ao menos ressalvas.\n' +
    '5. Atualize a memória com memorizar, secao onde_parou, modo substituir: "Workflow concluído: <n> critérios avaliados e verificados, INVEST e veredito registrados; a sessão principal conclui."\n\n' +
    REGRAS_DE_ACHADO + '\n\n' +
    'Devolva, em até cinco frases: o veredito, as contradições com a base ou o código e as perguntas para o autor.',
  { label: 'Consolidar ' + analise, effort: esforco },
)

return {
  analise: analise,
  retomada: retomada,
  historia: separacao.historia === true,
  criterios: criterios.map(function (c) { return c.titulo }),
  ja_avaliados: separacao.ja_avaliados || [],
  achados: achados,
  contestados: contestados,
  criterios_sem_resultado: falharam,
  consolidacao: consolidado,
  proximo_passo: 'Chame concluir_analise com analise ' + analise + ', trate as pendências e responda no chat como o roteiro manda.',
}
