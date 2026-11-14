import { fireEvent, render, screen } from "@testing-library/react";
import { useRef } from "react";
import { DivisorColunas, LARGURA_MINIMA, LARGURA_PADRAO, useLarguraConversa } from "./DivisorColunas";

function Colunas() {
  const ref = useRef<HTMLDivElement>(null);
  const [largura, mudar] = useLarguraConversa();
  return (
    <div ref={ref}>
      <DivisorColunas colunas={ref} largura={largura} aoMudar={mudar} />
    </div>
  );
}

// O jsdom não tem PointerEvent: sem ele, o evento perde o clientX.
if (typeof window.PointerEvent === "undefined") {
  class PointerEventFalso extends MouseEvent {
    pointerId: number;
    constructor(tipo: string, init: PointerEventInit = {}) {
      super(tipo, init);
      this.pointerId = init.pointerId ?? 0;
    }
  }
  window.PointerEvent = PointerEventFalso as unknown as typeof PointerEvent;
}

describe("DivisorColunas", () => {
  beforeEach(() => {
    try {
      localStorage.clear();
    } catch {
      // jsdom sem armazenamento
    }
  });

  it("ajusta a largura da conversa pelo teclado e volta ao padrão no duplo clique", () => {
    render(<Colunas />);
    const divisor = screen.getByRole("separator", { name: "Largura da conversa" });
    expect(divisor).toHaveAttribute("aria-valuenow", String(LARGURA_PADRAO));
    fireEvent.keyDown(divisor, { key: "ArrowLeft" });
    expect(divisor).toHaveAttribute("aria-valuenow", String(LARGURA_PADRAO + 24));
    fireEvent.keyDown(divisor, { key: "End" });
    expect(divisor).toHaveAttribute("aria-valuenow", String(LARGURA_MINIMA));
    fireEvent.doubleClick(divisor);
    expect(divisor).toHaveAttribute("aria-valuenow", String(LARGURA_PADRAO));
  });

  it("arrastar para a esquerda alarga a conversa", () => {
    render(<Colunas />);
    const divisor = screen.getByRole("separator", { name: "Largura da conversa" });
    divisor.setPointerCapture = () => undefined;
    divisor.releasePointerCapture = () => undefined;
    fireEvent.pointerDown(divisor, { button: 0, clientX: 800, pointerId: 1 });
    fireEvent.pointerMove(divisor, { clientX: 700, pointerId: 1 });
    fireEvent.pointerUp(divisor, { clientX: 700, pointerId: 1 });
    expect(divisor).toHaveAttribute("aria-valuenow", String(LARGURA_PADRAO + 100));
  });
});
