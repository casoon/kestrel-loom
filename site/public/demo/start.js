// Demo page of the project site: the wrapper and the WASM package are copied
// next to this file by scripts/demo-assets.mjs, from js/ and pkg/ of the repository.
import { createChart } from './lib/js/kestrel-loom.js';

const $ = (id) => document.getElementById(id);
const status = $('demo-status');
const canvas = $('demo-chart');

// Deterministic candles, the same generator as demo/index.html in the repository:
// every load shows the same chart.
function candles(n, seed = 42) {
  let s = seed >>> 0;
  const rnd = () => {
    s ^= s << 13;
    s >>>= 0;
    s ^= s >> 17;
    s ^= s << 5;
    s >>>= 0;
    return s / 4294967296;
  };
  const out = [];
  let price = 100;
  let time = 1600000000; // Unix seconds
  for (let i = 0; i < n; i++) {
    const o = price;
    const c = Math.max(1, o + (rnd() - 0.48) * 0.9);
    out.push({
      time,
      o,
      h: Math.max(o, c) + rnd() * 0.5,
      l: Math.min(o, c) - rnd() * 0.5,
      c,
      v: 100 + rnd() * 900,
    });
    price = c;
    // A 49-hour trading pause every 200 bars: the axis positions by bar index,
    // so the pause leaves no gap.
    time += i > 0 && i % 200 === 0 ? 49 * 3600 : 300; // M5
  }
  return out;
}

// The site's theme: an explicit choice on <html data-theme>, else the system setting.
const media = window.matchMedia('(prefers-color-scheme: dark)');
const isDark = () => {
  const chosen = document.documentElement.dataset.theme;
  return chosen ? chosen === 'dark' : media.matches;
};

try {
  const chart = await createChart(canvas, { timeframe: '5m', dark: isDark() });
  const data = candles(300);
  chart.data.set(data);
  chart.view.fit();

  const followTheme = () => chart.view.theme(isDark());
  new MutationObserver(followTheme).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme'],
  });
  media.addEventListener('change', followTheme);

  const all = chart.indicators.available();
  const indicatorSelect = $('demo-indicator');
  for (const name of all) {
    const option = document.createElement('option');
    option.value = name;
    option.textContent = name;
    indicatorSelect.appendChild(option);
  }

  const added = [];
  indicatorSelect.addEventListener('change', () => {
    const name = indicatorSelect.value;
    if (!name) return;
    try {
      added.push(chart.indicators.add(name));
      status.textContent = `Added ${name} — one of ${all.length} indicators in the catalogue.`;
    } catch (err) {
      status.textContent = `indicators.add("${name}") was rejected: ${err}`;
    }
    indicatorSelect.value = '';
  });

  $('demo-clear').addEventListener('click', () => {
    for (const pane of added.splice(0)) chart.indicators.remove(pane);
    status.textContent = 'Indicators removed.';
  });

  const styleSelect = $('demo-style');
  styleSelect.addEventListener('change', () => {
    try {
      chart.style.set(styleSelect.value);
      status.textContent = `Chart type: ${styleSelect.selectedOptions[0].textContent}.`;
    } catch (err) {
      status.textContent = `style.set("${styleSelect.value}") was rejected: ${err}`;
    }
  });

  const artifacts = $('demo-artifacts');
  artifacts.addEventListener('change', () => {
    chart.indicators.artifacts(artifacts.checked);
    status.textContent = artifacts.checked
      ? 'Artifacts on — visible for indicators that report some, e.g. extended_volume_profile.'
      : 'Artifacts off.';
  });

  $('demo-fit').addEventListener('click', () => chart.view.fit());

  for (const id of ['demo-style', 'demo-indicator', 'demo-artifacts', 'demo-fit', 'demo-clear']) {
    $(id).disabled = false;
  }
  status.textContent = `Running: ${data.length} bars, ${all.length} indicators available.`;
} catch (err) {
  status.textContent = `The chart could not start: ${err}`;
  console.error(err);
}
