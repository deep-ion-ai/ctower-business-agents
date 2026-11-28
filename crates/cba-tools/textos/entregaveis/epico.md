## Como redigir o épico com quebra em histórias

O épico é **um item só**, com o objetivo da demanda e as histórias em que ela se divide. Registre o épico inteiro numa chamada de `registrar_entregavel`; registrar de novo substitui o anterior.

1. **Objetivo** (`objetivo`): o resultado de negócio da demanda, em um ou dois parágrafos, e o que muda em relação a como o sistema é hoje (pelas regras confirmadas).
2. **Histórias** (`historias`): uma lista, **na ordem sugerida de entrega**. Cada história é um objeto com os campos da história de usuário:
   - `titulo`, `papel` (Como), `acao` (Quero), `beneficio` (Para);
   - `criterios`: os critérios de aceite, cada um verificável, com valores concretos ("Dado um cliente PF com fatura ABERTA de R$ 300,00, quando pede 3 parcelas, então são geradas 3 parcelas de R$ 100,00");
   - `regras`: as regras confirmadas que a história muda ou de que depende (ao menos uma);
   - `depende_de` (opcional): as histórias anteriores de que ela depende, pelo rótulo da posição (`H1`, `H2`). Só vale apontar para uma história que vem antes: ordene para que a dependência venha primeiro.
3. **Quebre em fatias verticais**: cada história entrega valor sozinha (tela, regra e dado juntos), é pequena o bastante para uma sprint e pode ser testada. Prefira separar por regra ou por tipo de cliente a separar por camada (uma história "do banco" e outra "da tela" não são fatias).
4. **Cobertura**: juntas, as histórias citam **todas** as regras confirmadas da análise. A resposta do registro avisa a regra sem história ("RN-FAT-009 sem história no épico"): cubra-a, ou diga ao analista por que ficou de fora (e ponha em `fora_do_escopo`).
5. **Dependências** (`dependencias`, opcional): o que o épico precisa de fora dele (outra equipe, um parâmetro de produção, uma decisão do negócio).
6. **Riscos** (`riscos`, opcional): o que pode dar errado ou mudar o tamanho, como uma regra que vale também para outros casos, uma divergência entre camadas ou uma pergunta ainda em aberto.
7. **Fora do escopo** (`fora_do_escopo`, opcional): o que a demanda não cobre, para o refinamento não ampliar o épico.
