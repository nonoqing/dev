import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

class FakeElement {
  constructor() {
    this.attributes = new Map();
    this.children = [];
    this.classList = { add() {}, toggle() {} };
    this.style = { setProperty() {} };
    this.textContent = '';
    this.src = '';
  }

  getAttribute(name) {
    if (name === 'src') return this.src || null;
    return this.attributes.get(name) ?? null;
  }

  setAttribute(name, value) {
    this.attributes.set(name, String(value));
  }

  addEventListener() {}
  append() {}
  appendChild() {}
  focus() {}
  querySelector() { return null; }
}

const elements = new Map();
const documentElement = new FakeElement();
documentElement.lang = 'zh-CN';
const document = {
  documentElement,
  getElementById(id) {
    if (!elements.has(id)) elements.set(id, new FakeElement());
    return elements.get(id);
  },
  querySelectorAll() { return []; },
  createElement() { return new FakeElement(); },
};
const window = {
  app: { locale: 'en-US' },
  scrollTo() {},
};
const sandbox = {
  Blob,
  URL,
  console,
  document,
  localStorage: { getItem() { return null; }, setItem() {} },
  requestAnimationFrame(callback) { callback(); },
  setTimeout,
  clearTimeout,
  window,
};

const sourceUrl = new URL('../source/ui.js', import.meta.url);
const source = (await readFile(sourceUrl, 'utf8')).replace('void init();', '');
vm.runInNewContext(source, sandbox);

const api = window.__CAT_CHARACTER_TEST__;
const result = api.calculateResult(Array(32).fill(4));
const avatar = document.getElementById('result-avatar');
const title = document.getElementById('result-name');
const summary = document.getElementById('result-summary');
const traits = document.getElementById('result-code');

avatar.src = 'data:image/webp;base64,newly-selected-portrait';
title.textContent = '静谧随缘猫';
summary.textContent = '这是结果页当前可见的中文摘要。';
traits.children = ['观察', '守成', '灵活', '松弛'].map((textContent) => ({ textContent }));

const zhSnapshot = api.createCardSnapshot(result, '团子');
assert.equal(zhSnapshot.locale, 'zh-CN');
assert.equal(zhSnapshot.profileTitle, '静谧随缘猫');
assert.equal(zhSnapshot.profileSummary, '这是结果页当前可见的中文摘要。');
assert.equal(zhSnapshot.portraitUrl, avatar.src);
assert.deepEqual(Array.from(zhSnapshot.traits), ['观察', '守成', '灵活', '松弛']);

documentElement.lang = 'en-US';
window.app.locale = 'zh-CN';
title.textContent = 'Serene Go-with-the-flow Cat';
summary.textContent = 'This is the summary currently visible on the result page.';
traits.children = ['Observing', 'Familiarity-seeking', 'Flexible', 'Easygoing'].map((textContent) => ({ textContent }));

const enSnapshot = api.createCardSnapshot(result, 'Tuanzi');
assert.equal(enSnapshot.locale, 'en-US');
assert.equal(enSnapshot.profileTitle, 'Serene Go-with-the-flow Cat');
assert.equal(enSnapshot.profileSummary, 'This is the summary currently visible on the result page.');
assert.equal(enSnapshot.portraitUrl, avatar.src);
assert.deepEqual(Array.from(enSnapshot.traits), ['Observing', 'Familiarity-seeking', 'Flexible', 'Easygoing']);

console.log('Result snapshots follow the visible language, text, and portrait.');
