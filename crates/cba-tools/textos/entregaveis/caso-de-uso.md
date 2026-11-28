## Como redigir o caso de uso

O caso de uso descreve **o que o sistema faz hoje** (ou, na proposta de uma demanda, o que passa a fazer), na visão de quem usa: atores, passos e resultados. Linguagem de negócio, sem nome de classe, tabela ou procedure; o código fica nas evidências das regras.

1. **Atores** (`atores`): quem inicia e quem participa (o papel, não a pessoa): "Atendente", "Supervisor", "Lote de cobrança". Parta dos atores do mapa e das telas dos pontos de entrada.
2. **Gatilho** (`gatilho`, opcional): o que faz o caso de uso começar, numa linha.
3. **Pré-condições** (`pre_condicoes`): o que precisa ser verdade antes do primeiro passo ("A fatura está ABERTA"), com a regra entre parênteses quando vier de uma.
4. **Fluxo principal** (`fluxo_principal`): o caminho que dá certo, uma etapa por item, na ordem, cada uma uma ação do ator ou uma resposta do sistema. **Toda etapa cita no próprio texto a regra que a sustenta**, entre parênteses: "O sistema confere que a fatura está ABERTA (RN-FAT-010)." Uma etapa que nenhuma regra sustenta não entra (ou junte-a à etapa seguinte, que tem regra). Não numere: o app numera.
5. **Fluxos alternativos** (`fluxos_alternativos`): os caminhos válidos que fogem do principal, um por item, dizendo em que etapa começam e como terminam: "Na etapa 3, se o cliente é órgão público, o sistema não exige valor mínimo de parcela e segue para a etapa 4 (RN-FAT-009)." Procure ao menos um: tipos de cliente, perfis, parâmetros e exceções de regra costumam gerar alternativos. O app os numera FA1, FA2...
6. **Exceções** (`excecoes`): as recusas e os erros, com a mensagem ou o efeito para o ator: "Na etapa 2, se a fatura não está ABERTA, o sistema recusa com 'Somente fatura ABERTA pode ser parcelada' e o caso de uso termina (RN-FAT-010)." O app as numera FE1, FE2...
7. **Pós-condições** (`pos_condicoes`): o que fica gravado ou muda quando o fluxo principal termina.
8. **Regras referenciadas** (`regras`): todas as regras que o caso de uso usa. Cite só as regras da lista do roteiro; uma regra que não está nela é recusada.
9. O que o código não responde (uma regra a revalidar, uma pergunta em aberto) não vira etapa: deixe a dúvida no objetivo ou fora do documento, e avise o analista.
10. Sem nenhuma regra na lista, não há como citar a origem das etapas: não registre. Diga ao analista que o caso de uso precisa das regras confirmadas da funcionalidade (uma explicação ou análise aprovada) e encerre.
