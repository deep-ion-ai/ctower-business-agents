export const meta = {
  name: 'cba-impacto',
  description: 'Análise de impacto de uma demanda pelo CBA, com investigação por frente e verificação independente',
  whenToUse: 'Só quando o pedido do analista manda chamar o workflow cba-impacto para uma análise A-NNNN do ctower.',
  phases: [
    { title: 'Preparar', detail: 'retoma a análise e separa as frentes de investigação' },
    { title: 'Investigar', detail: 'um agente por frente' },
    { title: 'Verificar', detail: 'um cético independente por frente' },
    { title: 'Consolidar', detail: 'abrangência, riscos transversais e onde parou' },
  ],
}

// Workflow padrão da análise de impacto do CBA (spec
// docs/specs/2026-11-25-workflows-nas-analises.md, seção 3.1; #371).
//
// args: "A-0042" (só o id), "A-0042 retomada" (conversa nova sobre uma análise já
// começada) ou { analise: "A-0042", retomada: false, limites: { frentes: 6,
// esforco: "medium", esforcoVerificacao: "low" } }. Os limites ausentes usam os
// padrões abaixo. O modelo dos agentes não é fixado: eles herdam o da sessão.
//
// Os agentes registram direto na análise pelas ferramentas do servidor MCP cba, as
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
const frentesNoMaximo = Math.max(1, Math.min(12, Number(limites.frentes) || 6))
const esforco = limites.esforco || 'medium'
const esforcoVerificacao = limites.esforcoVerificacao || 'low'

const COMUM =
  'Você é um agente do workflow cba-impacto, na análise ' + analise + ' do ctower. ' +
  'As ferramentas do servidor MCP cba (mcp__cba__...) carregam-se com ToolSearch; passe sempre analise ' + analise + '. ' +
  'Você não altera arquivos do projeto: só lê o código (Read, Grep, Glob e as ferramentas de consulta do cba: mapa, estrutura, referencias, schema, conhecimento, historico_do_trecho). ' +
  'Não chame iniciar_analise nem concluir_analise, e não siga o roteiro que retomar_analise devolve: siga só este pedido. ' +
  'Escreva para quem não lê código, em português.'

const REGRAS_DE_REGISTRO =
  'Registre com afirmar, uma regra ou fato por chamada, com a confiança honesta (confirmado só com evidência direta; inferido para dedução a partir das evidências listadas; a_confirmar quando o código não basta, o que já cria a pergunta). ' +
  'Cada evidência é o arquivo relativo à raiz do projeto, as linhas e o trecho copiado do código exatamente como está; evidência recusada volta com o motivo: corrija e chame de novo. Documentação não é evidência: o código é. ' +
  'Seções da análise de impacto: demanda_como_entendida, como_e_hoje, funcionalidades, dados, integracoes, documentos_e_telas, dados_pessoais_e_rnf, riscos. ' +
  'Premissa do solicitante vai em demanda_como_entendida no formato: "Premissa do solicitante: <o que ele afirmou ou supôs>. Verificação: <confere | não confere | confere em parte>. <o que o código mostra, com valores>." ' +
  'Em como_e_hoje, as regras atuais com valores e condições. A regra criada de propósito por uma demanda anterior vai em como_e_hoje com a chave, a data e a intenção da demanda, e o hash do commit em origens do afirmar (complementa a evidência de código, não a substitui). No que muda (funcionalidades, dados, integracoes, documentos_e_telas), cada item com o arquivo ou a tabela onde a mudança acontece; siga os valores até a origem (constante, parâmetro, configuração) e confira as outras camadas (tela, serviço, banco, lotes), onde a regra pode estar repetida, fixa ou diferente. ' +
  'Em dados_pessoais_e_rnf, só com evidência: campo pessoal (use schema com a tabela; o indício não é prova) no formato "Campo <TABELA.COLUNA>: <categoria> (dado pessoal | dado pessoal sensível). Finalidade aparente: ... Onde aparece: ...", e auditoria, desempenho e acesso afetados. ' +
  'Quando a mesma mudança se repete em muitos lugares, uma afirmação para o grupo, com a contagem e dois ou três exemplos citados. ' +
  'O que o código não responde (valores em tabela de parâmetros, jobs agendados, sistemas vizinhos, volume e janela) vira perguntar, escrita para o solicitante. ' +
  'Escolher a regra é do negócio: quando a correção exige escolher uma regra (manter ou revogar a de uma demanda anterior, qual valor vale, para que público), não proponha a regra em funcionalidades; registre a decisão com perguntar, tipo "decisao", com 2 a 4 opcoes, cada uma com o efeito de escolhê-la ("revoga a regra da demanda X", "mantém a exceção para Y"), e a recomendacao, se houver. ' +
  'Hipótese não vira escopo: uma mudança que depende de hipótese não verificada no código ("pode estar", "se houver", "opcional", "preventiva") não entra em funcionalidades; verifique-a ou registre com perguntar a dúvida, com o que a confirmaria, ou a decisão, com as opções. ' +
  'Termos de negócio que você ligou ao código vão para propor_termo. Antes de registrar, confira com retomar_analise se outro agente já registrou o mesmo fato: não registre de novo.'

const FRENTES = {
  type: 'object',
  properties: {
    demanda: { type: 'string', description: 'A demanda como o solicitante escreveu (a entrada da análise), sem resumir.' },
    premissas: {
      type: 'array',
      items: { type: 'string' },
      description: 'Cada afirmação contida na demanda, explícita ou implícita, a verificar no código.',
    },
    frentes: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          titulo: { type: 'string', description: 'Título curto e único da frente.' },
          foco: { type: 'string', description: 'O que investigar: as premissas desta frente, as regras, as camadas e o que pode mudar.' },
          pistas: { type: 'array', items: { type: 'string' }, description: 'Arquivos, rotinas, tabelas ou telas por onde começar.' },
        },
        required: ['titulo', 'foco'],
      },
    },
    ja_investigadas: { type: 'array', items: { type: 'string' }, description: 'Na retomada: as frentes que a memória já tem como investigadas.' },
  },
  required: ['demanda', 'premissas', 'frentes'],
}

const INVESTIGADA = {
  type: 'object',
  properties: {
    afirmacoes: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          numero: { type: 'integer' },
          secao: { type: 'string' },
          texto: { type: 'string' },
          evidencias: { type: 'array', items: { type: 'string' }, description: 'arquivo:linha_inicial-linha_final de cada evidência.' },
        },
        required: ['numero', 'secao', 'texto'],
      },
      description: 'As afirmações que você registrou nesta frente (o número vem da resposta de afirmar).',
    },
    perguntas: { type: 'integer', description: 'Quantas perguntas você registrou.' },
    resumo: { type: 'string', description: 'O que a frente concluiu, em até três frases.' },
  },
  required: ['afirmacoes', 'resumo'],
}

const VERIFICADA = {
  type: 'object',
  properties: {
    verificadas: { type: 'integer' },
    contestadas: {
      type: 'array',
      items: {
        type: 'object',
        properties: { numero: { type: 'integer' }, motivo: { type: 'string' } },
        required: ['numero', 'motivo'],
      },
    },
  },
  required: ['verificadas', 'contestadas'],
}

phase('Preparar')
const preparo = await agent(
  COMUM + '\n\n' +
    'Fase Preparar. Chame retomar_analise para ler a demanda (a entrada), o que já foi registrado e a memória da análise. ' +
    'Liste cada afirmação contida na demanda, explícita ou implícita: o que o solicitante diz que o sistema faz ou não faz hoje, o que ele supõe que basta mudar ("acho que", "basta", "hoje não tem") e o que ele pede de novo (valores, limites, públicos); um pedido como "permitir X" pressupõe que X hoje não é permitido. ' +
    'Localize as funcionalidades afetadas, sem investigar a fundo: consulte a base (conhecimento) e o código (mapa, conhecimento, referencias, estrutura). ' +
    'Separe a investigação em até ' + frentesNoMaximo + ' frentes independentes (por exemplo: a regra principal e as premissas dela; as outras camadas onde a regra se repete; dados e migração; integrações e lotes; relatórios, documentos e telas; dados pessoais e requisitos não funcionais). Menos frentes é melhor quando a demanda é pequena: não crie frente sem o que investigar. Cada premissa fica em alguma frente. ' +
    'Quando a demanda trata de um estado errado (um status, uma situação, uma flag com um valor que não deveria ter), crie uma frente com o título "Caminhos que gravam <o estado>" para fechar o conjunto de quem grava esse estado: a rota que a tela usa é só um dos caminhos. ' +
    (retomada
      ? 'Esta é uma retomada: a seção investigado da memória diz quais frentes já foram investigadas (itens "Frente <título>: ..."). Devolva em frentes só as que ainda faltam, com os mesmos títulos, e as já investigadas em ja_investigadas. Não grave as frentes de novo na memória. '
      : 'Grave as frentes na memória com memorizar, secao contexto, um item: "Frentes do workflow: 1. <título>: <foco>; 2. ...". ') +
    'Não registre afirmações nem perguntas nesta fase.',
  { label: 'Preparar ' + analise, schema: FRENTES, effort: esforco },
)
if (!preparo) {
  throw new Error('a fase Preparar não devolveu as frentes')
}

let frentes = (preparo.frentes || []).filter(function (f) { return f && f.titulo })
if (frentes.length > frentesNoMaximo) {
  log('Preparar devolveu ' + frentes.length + ' frentes; ficam as ' + frentesNoMaximo + ' primeiras (limites.frentes).')
  frentes = frentes.slice(0, frentesNoMaximo)
}
if (retomada && (preparo.ja_investigadas || []).length > 0) {
  log('Retomada: ' + preparo.ja_investigadas.length + ' frente(s) já investigada(s) não são refeitas.')
}

const premissas = (preparo.premissas || []).map(function (p, i) { return (i + 1) + '. ' + p }).join('\n')

const resultados = await pipeline(
  frentes,
  function (_, frente) {
    return agent(
      COMUM + '\n\n' +
        'Fase Investigar, frente "' + frente.titulo + '". Outros agentes investigam as outras frentes em paralelo; fique na sua.\n\n' +
        'Demanda, como o solicitante escreveu:\n' + preparo.demanda + '\n\n' +
        'Premissas do solicitante (verifique as que tocam a sua frente):\n' + premissas + '\n\n' +
        'Foco da frente: ' + frente.foco + '\n' +
        (frente.pistas && frente.pistas.length ? 'Por onde começar: ' + frente.pistas.join('; ') + '\n' : '') +
        '\nVerifique no código cada premissa e cada regra da frente, uma por uma; nenhuma afirmação do solicitante entra como verdade sem verificação. Levante como é hoje e o que muda (funcionalidades, dados, inclusive os já gravados no formato atual, integrações, documentos e telas), os dados pessoais e os riscos da frente. ' +
        'Depois de descrever como é hoje, consulte a origem de cada regra que a demanda muda: chame historico_do_trecho com o arquivo e as linhas da evidência (ou com texto, o identificador da regra). O commit mais antigo é a origem; veja também os que mudaram a regra depois. Quando a mensagem cita uma demanda (uma chave como DEM-12 ou #12) que pediu esse comportamento de propósito, registre isso em como_e_hoje, com a chave e o hash do commit em origens, e registre com perguntar, tipo "decisao", citando a chave, se a regra da demanda continua valendo, com as opções (manter ou revogar a regra) e o efeito de cada uma: a mudança pode revogá-la sem o negócio saber. Commit sem chave é só ajuste: não invente a demanda, e não cite nomes de pessoas. ' +
        'Se a frente trata de quem grava um estado (status, situação, flag), liste todos os caminhos que gravam ou alteram esse estado: parta do campo (referencias com o campo e tipos ["escrita"], ou com a tabela e tipos ["mencao"] para o SQL embutido no backend) ou do método de escrita e suba com referencias e profundidade 2 ou 3 até os pontos de entrada, inclusive chamadas internas, lotes e edições de outros registros; em cada rotina que grava, leia o que ela valida antes e se quem chama pode desligar a validação. Registre em como_e_hoje uma afirmação "Caminhos que gravam <estado>: 1. <ponto de entrada> → <rotina>: valida <o quê>; 2. ...", com a evidência de cada gravação, dizendo se a lista é completa ou o que ficou aberto (árvore cortada, rotina sem chamador no mapa). Com mais de um caminho, cada um vira item em riscos (ou em funcionalidades, quando a demanda muda aquele caminho), dizendo o que valida e o que não valida. A suposição do solicitante sobre o caminho ("deve ser a rota antiga") é premissa: verifique-a. ' +
        'Não registre a abrangência nem a afirmação final "Demanda como entendida": isso é da fase Consolidar.\n\n' +
        REGRAS_DE_REGISTRO + '\n\n' +
        'Ao terminar, grave com memorizar, secao investigado, um item: "Frente ' + frente.titulo + ': <o que concluiu, com os arquivos principais>". ' +
        'Devolva as afirmações que você registrou, com o número que afirmar devolveu, a seção, o texto e as evidências.',
      { label: 'Investigar: ' + frente.titulo, phase: 'Investigar', schema: INVESTIGADA, effort: esforco },
    )
  },
  function (investigada, frente) {
    if (!investigada) {
      return { afirmacoes: 0, verificacao: null, semResultado: true }
    }
    const afirmacoes = investigada.afirmacoes || []
    if (afirmacoes.length === 0) {
      return { afirmacoes: 0, verificacao: null }
    }
    const lista = afirmacoes
      .map(function (a) {
        return a.numero + ' [' + a.secao + '] ' + a.texto + (a.evidencias && a.evidencias.length ? ' Evidências: ' + a.evidencias.join(', ') : '')
      })
      .join('\n')
    return agent(
      COMUM + '\n\n' +
        'Fase Verificar, frente "' + frente.titulo + '". Você é um verificador cético e independente: não foi você quem registrou estas afirmações. ' +
        'Para cada uma, releia o código citado (Read nas linhas da evidência, e em volta delas) e tente refutá-la: o trecho é mesmo desta regra? o valor e a condição conferem? outra camada (tela, serviço, banco, lote) contradiz? a confiança é honesta? ' +
        'Quando a afirmação não se sustenta, chame contestar_afirmacao com o número, o motivo em linguagem de negócio, com valores, e, se achar, a evidência do contrário. ' +
        'Quando uma afirmação lista os caminhos que gravam um estado ("Caminhos que gravam ..."), confira se a lista ficou fechada: chame referencias com o campo ou o método de escrita e profundidade 2 ou 3 e procure um caminho que grava o estado e não está na lista (chamada interna, lote, edição de outro registro) ou um caminho dado como validado que pode ter a validação desligada por quem chama. Faltando algum, conteste a afirmação com o caminho que faltou, em linguagem de negócio, e a evidência da gravação. ' +
        'Hipótese não vira escopo: uma afirmação de funcionalidades que depende de hipótese não verificada (o código citado não a confirma: "pode estar", "se houver", "opcional", "preventiva") ou que escolhe uma regra no lugar do negócio não se sustenta como escopo. Conteste-a com o motivo e registre com perguntar, ligada a ela (afirmacao: o número), a dúvida, com o que confirmaria a hipótese, ou a decisão (tipo "decisao", com 2 a 4 opcoes e o efeito de cada uma). ' +
        'Afirmação que se sustenta não precisa de chamada. Fora esse caso, não registre afirmações nem perguntas, e não conteste por estilo ou redação.\n\n' +
        'Afirmações da frente:\n' + lista + '\n\n' +
        'Devolva quantas verificou e as que contestou.',
      { label: 'Verificar: ' + frente.titulo, phase: 'Verificar', schema: VERIFICADA, effort: esforcoVerificacao },
    ).then(function (v) {
      return { afirmacoes: afirmacoes.length, verificacao: v }
    })
  },
)

const feitas = resultados.filter(Boolean)
const afirmacoes = feitas.reduce(function (n, r) { return n + (r.afirmacoes || 0) }, 0)
const contestadas = feitas.reduce(function (n, r) {
  return n + ((r.verificacao && r.verificacao.contestadas) || []).length
}, 0)
const falharam = resultados.filter(function (r) { return !r || r.semResultado }).length
if (falharam > 0) {
  log(falharam + ' frente(s) sem resultado (agente com erro ou pulado); a sessão completa o que faltar.')
}

phase('Consolidar')
const consolidado = await agent(
  COMUM + '\n\n' +
    'Fase Consolidar. As frentes foram investigadas e verificadas por outros agentes. Chame retomar_analise para ver tudo o que foi registrado. ' +
    'As afirmações a revalidar, contestadas pela verificação independente, ficam para o analista decidir: não as apague, não as registre de novo e não se apoie nelas. ' +
    'Faça só o que falta, sem repetir o que já está registrado:\n' +
    '1. Se o mesmo fato aparece em mais de uma frente, não registre de novo; quando a mesma mudança se repete em muitos lugares sem uma afirmação de grupo, registre uma, com a contagem e dois ou três exemplos citados.\n' +
    '2. Riscos transversais (riscos): dados antigos no formato atual, integrações e lotes que dependem do comportamento atual, regras repetidas em mais de uma camada que podem ficar divergentes, outros públicos afetados pela mesma mudança, e cada caminho que grava o estado da demanda sem validar, se ainda não estiver em riscos.\n' +
    '3. Feche demanda_como_entendida com uma afirmação "Demanda como entendida: <o que de fato precisa mudar>", que já incorpora as correções das premissas.\n' +
    '4. Estime a abrangência (abrangencia) numa afirmação inferido que começa por "Pequena:", "Média:" ou "Grande:", com a justificativa e a contagem de lugares afetados, citando as evidências principais (pequena: um lugar ou um parâmetro, sem mudar estrutura de dados; média: algumas camadas ou uma regra repetida, sem migrar dados; grande: estrutura de dados com migração, integrações externas ou muitos lugares).\n' +
    '5. Se faltar afirmação em como_e_hoje, registre as regras atuais do assunto.\n' +
    '6. Decisões para o negócio: se alguma escolha de regra ficou em funcionalidades como proposta, ou se uma regra criada por demanda anterior é revogada pela mudança sem decisão registrada, registre a decisão com perguntar, tipo "decisao", com as opções e o efeito de cada uma.\n' +
    '7. Atualize a memória com memorizar, secao onde_parou, modo substituir: "Workflow concluído: <n> frentes investigadas e verificadas; a sessão principal conclui."\n\n' +
    REGRAS_DE_REGISTRO + '\n\n' +
    'Devolva, em até cinco frases: as decisões para o negócio, a demanda como entendida, a abrangência e os principais riscos.',
  { label: 'Consolidar ' + analise, effort: esforco },
)

return {
  analise: analise,
  retomada: retomada,
  frentes: frentes.map(function (f) { return f.titulo }),
  ja_investigadas: preparo.ja_investigadas || [],
  afirmacoes: afirmacoes,
  contestadas: contestadas,
  frentes_sem_resultado: falharam,
  consolidacao: consolidado,
  proximo_passo: 'Chame concluir_analise com analise ' + analise + ', trate as pendências e responda no chat como o roteiro manda.',
}
