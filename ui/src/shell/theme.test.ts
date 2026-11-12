import { act, renderHook } from "@testing-library/react";
import boot from "../../public/theme-boot.js?raw";
import { applyTheme, readThemePref, THEME_KEY, useTheme } from "./theme";

/** Simula o `prefers-color-scheme` do sistema. */
function sistemaEscuro(escuro: boolean) {
  const ouvintes: (() => void)[] = [];
  const mq = {
    get matches() {
      return escuro;
    },
    addEventListener: (_: string, f: () => void) => ouvintes.push(f),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal("matchMedia", vi.fn(() => mq));
  return {
    mudar(v: boolean) {
      escuro = v;
      ouvintes.forEach((f) => f());
    },
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
  delete document.documentElement.dataset.theme;
});

describe("tema", () => {
  it("sem preferência gravada, segue o sistema", () => {
    expect(readThemePref()).toBe("system");
    sistemaEscuro(false);
    expect(applyTheme("system")).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("grava a escolha e aplica no <html>", () => {
    sistemaEscuro(true);
    const { result } = renderHook(() => useTheme());
    act(() => result.current[1]("light"));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem(THEME_KEY)).toBe("light");
    expect(result.current[0]).toBe("light");
  });

  it("em \"do sistema\", acompanha a troca do sistema", () => {
    const sistema = sistemaEscuro(true);
    const { result } = renderHook(() => useTheme());
    act(() => result.current[1]("system"));
    expect(document.documentElement.dataset.theme).toBe("dark");
    act(() => sistema.mudar(false));
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("o theme-boot.js aplica a preferência gravada antes do primeiro quadro", () => {
    sistemaEscuro(true);
    localStorage.setItem(THEME_KEY, "light");
    new Function(boot)();
    expect(document.documentElement.dataset.theme).toBe("light");

    localStorage.setItem(THEME_KEY, "system");
    new Function(boot)();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
