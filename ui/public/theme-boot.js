// Tema antes do primeiro quadro (#85), para não piscar o tema errado.
// Origem: deep-ion-ai/ctower-agents, ui/public/theme-boot.js (commit e458ff6,
// Apache-2.0). Aqui a preferência vive só no localStorage (src/shell/theme.ts).
(function () {
  var pref = null;
  try {
    pref = localStorage.getItem("cba.theme");
  } catch (e) {}
  pref = pref || "system";
  var light =
    pref === "light" ||
    (pref === "system" && !!window.matchMedia && !window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = light ? "light" : "dark";
})();
