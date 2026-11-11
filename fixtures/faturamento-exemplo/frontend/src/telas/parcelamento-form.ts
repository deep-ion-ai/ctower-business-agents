// Tela de parcelamento de fatura (portal do cliente).
// Valida o pedido antes de enviar para o backend.

export type TipoCliente = 'PF' | 'PJ' | 'PUB';

export interface PedidoParcelamento {
  idFatura: number;
  tipoCliente: TipoCliente;
  valorFatura: number;
  quantidadeParcelas: number;
}

export interface ErroValidacao {
  campo: keyof PedidoParcelamento;
  mensagem: string;
}

const MIN_PARCELAS = 2;
const MAX_PARCELAS = 12;
const VALOR_MINIMO_PARCELA = 50.0;

export function validarParcelamento(pedido: PedidoParcelamento): ErroValidacao[] {
  const erros: ErroValidacao[] = [];

  if (pedido.tipoCliente === 'PF') {
    erros.push({
      campo: 'tipoCliente',
      mensagem: 'Parcelamento disponível apenas para empresas e órgãos públicos.',
    });
    return erros;
  }

  if (
    !Number.isInteger(pedido.quantidadeParcelas) ||
    pedido.quantidadeParcelas < MIN_PARCELAS ||
    pedido.quantidadeParcelas > MAX_PARCELAS
  ) {
    erros.push({
      campo: 'quantidadeParcelas',
      mensagem: `Escolha entre ${MIN_PARCELAS} e ${MAX_PARCELAS} parcelas.`,
    });
  }

  const valorParcela = pedido.valorFatura / pedido.quantidadeParcelas;
  if (pedido.tipoCliente !== 'PUB' && valorParcela < VALOR_MINIMO_PARCELA) {
    erros.push({
      campo: 'quantidadeParcelas',
      mensagem: `Cada parcela deve ser de pelo menos R$ ${VALOR_MINIMO_PARCELA.toFixed(2)}.`,
    });
  }

  return erros;
}

export function opcoesDeParcelas(): number[] {
  const opcoes: number[] = [];
  for (let n = MIN_PARCELAS; n <= MAX_PARCELAS; n++) {
    opcoes.push(n);
  }
  return opcoes;
}

export async function enviarParcelamento(pedido: PedidoParcelamento): Promise<void> {
  const erros = validarParcelamento(pedido);
  if (erros.length > 0) {
    throw new Error(erros.map((e) => e.mensagem).join(' '));
  }

  const resposta = await fetch(`/api/faturas/${pedido.idFatura}/parcelamento`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ quantidade: pedido.quantidadeParcelas }),
  });

  if (!resposta.ok) {
    const corpo = await resposta.json().catch(() => ({ mensagem: 'Erro ao parcelar a fatura.' }));
    throw new Error(corpo.mensagem);
  }
}
