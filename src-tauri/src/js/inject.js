// Johnny Mecro page agent: recorder + fingerprint + resolver + DOM distiller + Set-of-Mark overlay.
// Injected through CDP (Page.addScriptToEvaluateOnNewDocument and Runtime.evaluate).
(function () {
  if (window.__jm) return;
  if (window.top !== window) return; // top frame only

  var REC_SEL =
    'a[href],button,input,select,textarea,summary,label,[role="button"],[role="link"],[role="tab"],' +
    '[role="menuitem"],[role="option"],[role="checkbox"],[role="radio"],[role="switch"],[role="combobox"],' +
    '[role="textbox"],[role="searchbox"],[onclick],[contenteditable=""],[contenteditable="true"]';
  var DISTILL_SEL =
    'a[href],button,input,select,textarea,summary,[role="button"],[role="link"],[role="tab"],' +
    '[role="menuitem"],[role="option"],[role="checkbox"],[role="radio"],[role="switch"],[role="combobox"],' +
    '[role="textbox"],[role="searchbox"],[onclick],[contenteditable=""],[contenteditable="true"],' +
    '[tabindex]:not([tabindex="-1"])';

  function norm(s) {
    return (s == null ? '' : String(s)).replace(/\s+/g, ' ').trim();
  }
  function cut(s, n) {
    s = norm(s);
    return s.length > n ? s.slice(0, n) : s;
  }
  function esc(s) {
    return window.CSS && CSS.escape ? CSS.escape(s) : String(s).replace(/([^\w-])/g, '\\$1');
  }
  function q(s) {
    return String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  }
  function stableId(id) {
    return id && !/\d{4,}/.test(id) && !/^(:r|ember|react-|radix-)/.test(id) && id.length < 60;
  }
  function unique(sel, el) {
    try {
      var r = document.querySelectorAll(sel);
      return r.length === 1 && r[0] === el;
    } catch (e) {
      return false;
    }
  }
  function isEditable(el) {
    if (!el || el.nodeType !== 1) return false;
    var t = el.tagName;
    if (t === 'TEXTAREA') return true;
    if (t === 'INPUT') {
      var ty = (el.getAttribute('type') || 'text').toLowerCase();
      return ['checkbox', 'radio', 'button', 'submit', 'reset', 'file', 'image', 'range', 'color', 'hidden'].indexOf(ty) < 0;
    }
    return !!el.isContentEditable;
  }

  function implicitRole(el) {
    var explicit = el.getAttribute('role');
    if (explicit) return explicit;
    var t = el.tagName.toLowerCase();
    if (t === 'a') return el.hasAttribute('href') ? 'link' : 'generic';
    if (t === 'button' || t === 'summary') return 'button';
    if (t === 'select') return 'combobox';
    if (t === 'textarea') return 'textbox';
    if (t === 'img') return 'img';
    if (/^h[1-6]$/.test(t)) return 'heading';
    if (t === 'input') {
      var ty = (el.getAttribute('type') || 'text').toLowerCase();
      if (ty === 'checkbox') return 'checkbox';
      if (ty === 'radio') return 'radio';
      if (ty === 'button' || ty === 'submit' || ty === 'reset' || ty === 'image') return 'button';
      if (ty === 'search') return 'searchbox';
      if (ty === 'range') return 'slider';
      return 'textbox';
    }
    if (el.isContentEditable) return 'textbox';
    return 'generic';
  }

  function axName(el) {
    var v = el.getAttribute('aria-label');
    if (norm(v)) return cut(v, 80);
    var lb = el.getAttribute('aria-labelledby');
    if (lb) {
      var txt = lb
        .split(/\s+/)
        .map(function (id) {
          var e = document.getElementById(id);
          return e ? e.textContent : '';
        })
        .join(' ');
      if (norm(txt)) return cut(txt, 80);
    }
    var t = el.tagName;
    if (t === 'INPUT' || t === 'SELECT' || t === 'TEXTAREA') {
      if (el.id) {
        try {
          var l = document.querySelector('label[for="' + q(el.id) + '"]');
          if (l && norm(l.textContent)) return cut(l.textContent, 80);
        } catch (e) {}
      }
      var wrap = el.closest('label');
      if (wrap && norm(wrap.textContent)) return cut(wrap.textContent, 80);
      var ty = (el.getAttribute('type') || '').toLowerCase();
      if (t === 'INPUT' && (ty === 'submit' || ty === 'button' || ty === 'reset') && norm(el.value)) return cut(el.value, 80);
      if (norm(el.getAttribute('placeholder'))) return cut(el.getAttribute('placeholder'), 80);
      if (norm(el.getAttribute('title'))) return cut(el.getAttribute('title'), 80);
      return cut(el.getAttribute('name') || '', 80);
    }
    if (t === 'IMG') return cut(el.getAttribute('alt') || el.getAttribute('title') || '', 80);
    var text = elText(el);
    if (text) return cut(text, 80);
    var img = el.querySelector && el.querySelector('img[alt]');
    if (img && norm(img.getAttribute('alt'))) return cut(img.getAttribute('alt'), 80);
    return cut(el.getAttribute('title') || '', 80);
  }

  function elText(el) {
    var t = el.tagName;
    if (t === 'INPUT') {
      var ty = (el.getAttribute('type') || '').toLowerCase();
      if (ty === 'submit' || ty === 'button' || ty === 'reset') return cut(el.value, 100);
      return '';
    }
    if (t === 'SELECT' || t === 'TEXTAREA') return '';
    return cut(el.innerText != null ? el.innerText : el.textContent, 100);
  }

  function cssPath(el) {
    var tag = el.tagName.toLowerCase();
    if (stableId(el.id) && unique('#' + esc(el.id), el)) return '#' + esc(el.id);
    var attrs = ['data-testid', 'data-test', 'data-qa', 'data-cy', 'name', 'aria-label', 'placeholder', 'title'];
    for (var i = 0; i < attrs.length; i++) {
      var v = el.getAttribute(attrs[i]);
      if (v && v.length < 80) {
        var s = tag + '[' + attrs[i] + '="' + q(v) + '"]';
        if (unique(s, el)) return s;
      }
    }
    var parts = [];
    var cur = el;
    while (cur && cur.nodeType === 1 && cur !== document.documentElement) {
      if (stableId(cur.id) && unique('#' + esc(cur.id), cur)) {
        parts.unshift('#' + esc(cur.id));
        break;
      }
      var part = cur.tagName.toLowerCase();
      var p = cur.parentElement;
      if (p) {
        var same = [];
        for (var k = 0; k < p.children.length; k++) if (p.children[k].tagName === cur.tagName) same.push(p.children[k]);
        if (same.length > 1) part += ':nth-of-type(' + (same.indexOf(cur) + 1) + ')';
      }
      parts.unshift(part);
      var sel = parts.join(' > ');
      if (parts.length >= 2 && unique(sel, el)) return sel;
      cur = p;
    }
    return parts.join(' > ');
  }

  function xpathOf(el) {
    var parts = [];
    var cur = el;
    while (cur && cur.nodeType === 1) {
      if (stableId(cur.id) && unique('#' + esc(cur.id), cur)) {
        parts.unshift('//*[@id="' + cur.id + '"]');
        return parts.join('/');
      }
      var idx = 1;
      var sib = cur.previousElementSibling;
      while (sib) {
        if (sib.tagName === cur.tagName) idx++;
        sib = sib.previousElementSibling;
      }
      parts.unshift(cur.tagName.toLowerCase() + '[' + idx + ']');
      cur = cur.parentElement;
    }
    return '/' + parts.join('/');
  }

  function contextText(el) {
    var p = el.parentElement;
    var hops = 0;
    while (p && hops < 3) {
      var t = cut(p.innerText || '', 140);
      if (t.length > elText(el).length + 4) return t;
      p = p.parentElement;
      hops++;
    }
    return '';
  }

  function fingerprint(el) {
    var r = el.getBoundingClientRect();
    var attrs = {};
    ['type', 'name', 'placeholder', 'href', 'value', 'title', 'class'].forEach(function (a) {
      if (a === 'value' && isEditable(el)) return;
      var v = el.getAttribute(a);
      if (v) attrs[a] = cut(v, 120);
    });
    return {
      primaryCss: cssPath(el),
      xpath: xpathOf(el),
      axRole: implicitRole(el),
      axName: axName(el),
      innerText: elText(el),
      contextText: contextText(el),
      tag: el.tagName.toLowerCase(),
      attrs: attrs,
      boundingBox: {
        x: Math.round(r.left),
        y: Math.round(r.top),
        width: Math.round(r.width),
        height: Math.round(r.height),
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
      },
    };
  }

  function visible(el) {
    if (!el || !el.isConnected) return false;
    var r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return false;
    if (el.checkVisibility) {
      try {
        return el.checkVisibility({ checkVisibilityCSS: true });
      } catch (e) {}
    }
    var s = getComputedStyle(el);
    return s.visibility !== 'hidden' && s.display !== 'none';
  }
  function enabled(el) {
    return !el.disabled && el.getAttribute('aria-disabled') !== 'true';
  }

  // Walk the document including open shadow roots.
  function walk(root, fn) {
    var kids = root.children || [];
    for (var i = 0; i < kids.length; i++) {
      var el = kids[i];
      var t = el.tagName;
      if (t === 'SCRIPT' || t === 'STYLE' || t === 'NOSCRIPT' || t === 'TEMPLATE' || el.id === '__jm_som') continue;
      var descend = fn(el);
      if (descend === false) continue;
      if (el.shadowRoot) walk(el.shadowRoot, fn);
      walk(el, fn);
    }
  }

  function pointOf(el) {
    var r = el.getBoundingClientRect();
    var inView = r.top >= 0 && r.left >= 0 && r.bottom <= window.innerHeight && r.right <= window.innerWidth;
    if (!inView) {
      try {
        el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
      } catch (e) {
        el.scrollIntoView();
      }
      r = el.getBoundingClientRect();
    }
    var x = Math.min(Math.max(r.left + r.width / 2, 1), window.innerWidth - 1);
    var y = Math.min(Math.max(r.top + r.height / 2, 1), window.innerHeight - 1);
    return { x: x, y: y };
  }

  function dist(el, bb) {
    if (!bb) return 0;
    var r = el.getBoundingClientRect();
    return Math.abs(r.left - bb.x) + Math.abs(r.top - bb.y);
  }

  // Tier 1 resolver: primaryCss -> axRole+axName -> xpath -> innerText
  function resolve(fp) {
    if (!fp) return { status: 'notfound' };
    var sawHidden = false;
    function accept(el, method) {
      if (!el || el.nodeType !== 1) return null;
      if (fp.tag && el.tagName.toLowerCase() !== fp.tag && (method === 'css' || method === 'xpath')) return null;
      if (!visible(el) || !enabled(el)) {
        sawHidden = true;
        return null;
      }
      J.last = el;
      var p = pointOf(el);
      return { status: 'ok', method: method, x: p.x, y: p.y };
    }
    var res;
    if (fp.primaryCss) {
      try {
        res = accept(document.querySelector(fp.primaryCss), 'css');
        if (res) return res;
      } catch (e) {}
    }
    if (fp.axName && fp.axRole) {
      var cands = [];
      walk(document, function (el) {
        if (implicitRole(el) === fp.axRole && axName(el) === fp.axName && visible(el) && enabled(el)) cands.push(el);
      });
      if (cands.length) {
        cands.sort(function (a, b) {
          return dist(a, fp.boundingBox) - dist(b, fp.boundingBox);
        });
        res = accept(cands[0], 'ax');
        if (res) return res;
      }
    }
    if (fp.xpath) {
      try {
        var x = document.evaluate(fp.xpath, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue;
        res = accept(x, 'xpath');
        if (res) return res;
      } catch (e) {}
    }
    if (fp.innerText) {
      var exact = [];
      var partial = [];
      walk(document, function (el) {
        if (!el.matches(REC_SEL) && el.tagName.toLowerCase() !== fp.tag) return;
        var t = elText(el);
        if (!t) return;
        if (t === fp.innerText) exact.push(el);
        else if (t.length < fp.innerText.length * 3 && t.indexOf(fp.innerText) >= 0) partial.push(el);
      });
      var list = exact.length ? exact : partial;
      list = list.filter(function (el) {
        return visible(el) && enabled(el);
      });
      if (list.length) {
        list.sort(function (a, b) {
          return dist(a, fp.boundingBox) - dist(b, fp.boundingBox);
        });
        res = accept(list[0], 'text');
        if (res) return res;
      }
    }
    return { status: sawHidden ? 'hidden' : 'notfound' };
  }

  // ---------- Module C: interactive DOM distiller ----------
  function attr(name, v, n) {
    v = cut(v, n || 60);
    return v ? ' ' + name + '="' + v.replace(/"/g, "'") + '"' : '';
  }
  function describe(el) {
    var tag = el.tagName.toLowerCase();
    var s = '<' + tag;
    var role = el.getAttribute('role');
    if (role) s += attr('role', role);
    if (tag === 'input') s += attr('type', el.getAttribute('type') || 'text');
    s += attr('name', el.getAttribute('name'), 30);
    var text = elText(el);
    if (text) s += attr('text', text, 80);
    var al = el.getAttribute('aria-label');
    if (al && norm(al) !== text) s += attr('aria-label', al);
    if (tag === 'input' || tag === 'textarea') {
      s += attr('placeholder', el.getAttribute('placeholder'));
      var ty = (el.getAttribute('type') || '').toLowerCase();
      if (ty === 'checkbox' || ty === 'radio') s += el.checked ? ' checked' : '';
      else if (ty === 'password') s += ' value="' + (el.value ? '***' : '') + '"';
      else if (ty !== 'submit' && ty !== 'button') s += ' value="' + cut(el.value, 40).replace(/"/g, "'") + '"';
      if (!text && !al && !el.getAttribute('placeholder')) {
        var nm = axName(el);
        if (nm) s += attr('label', nm);
      }
    }
    if (tag === 'select') {
      var o = el.options[el.selectedIndex];
      s += attr('value', o ? o.text : '');
      var opts = [];
      for (var i = 0; i < el.options.length && i < 12; i++) opts.push(cut(el.options[i].text, 20));
      s += attr('options', opts.join('|'), 200);
    }
    if (tag === 'a') {
      var h = el.getAttribute('href') || '';
      if (h && h.indexOf('javascript:') !== 0) {
        try {
          h = decodeURI(h);
        } catch (e) {}
        s += attr('href', h.length > 48 ? h.slice(0, 48) + '…' : h, 49);
      }
    }
    if (!text && !al) {
      s += attr('title', el.getAttribute('title'));
      var img = el.querySelector('img[alt]');
      if (img) s += attr('alt', img.getAttribute('alt'));
    }
    if (el.getAttribute('aria-expanded')) s += attr('aria-expanded', el.getAttribute('aria-expanded'));
    if (el.getAttribute('aria-selected') === 'true') s += ' selected';
    if (!enabled(el)) s += ' disabled';
    return s + ' />';
  }

  function distill(opts) {
    opts = opts || {};
    var max = opts.max || 220;
    var textBudget = opts.textBudget || 5000;
    var items = []; // {el?, text?, inView}
    var total = 0;
    var checkPointer = document.getElementsByTagName('*').length < 6000;
    var vh = window.innerHeight;
    walk(document, function (el) {
      if (el.getAttribute('aria-hidden') === 'true' && !el.matches(DISTILL_SEL)) return false;
      var inter = el.matches(DISTILL_SEL);
      if (!inter && checkPointer && el.children.length < 6) {
        var cs = getComputedStyle(el);
        if (cs.cursor === 'pointer') {
          var pe = el.parentElement;
          if (!pe || getComputedStyle(pe).cursor !== 'pointer') inter = true;
        }
      }
      if (inter) {
        if (el.tagName === 'INPUT' && (el.getAttribute('type') || '').toLowerCase() === 'hidden') return false;
        if (!visible(el)) return el.children.length > 0; // hidden wrapper may have visible custom children
        var r = el.getBoundingClientRect();
        items.push({ el: el, inView: r.bottom > -vh && r.top < vh * 2 });
        total++;
        return el.querySelector(DISTILL_SEL) ? undefined : false;
      }
      // direct text nodes of non-interactive elements
      if (el.closest && el.closest(DISTILL_SEL)) return;
      var own = '';
      for (var i = 0; i < el.childNodes.length; i++) {
        var n = el.childNodes[i];
        if (n.nodeType === 3) own += ' ' + n.nodeValue;
      }
      own = norm(own);
      if (own.length >= 2 && visible(el)) {
        var rr = el.getBoundingClientRect();
        items.push({ text: cut(own, 160), heading: /^H[1-4]$/.test(el.tagName), inView: rr.bottom > -vh && rr.top < vh * 2 });
      }
    });
    var truncated = false;
    if (total > max) {
      truncated = true;
      items = items.filter(function (it) {
        return it.inView;
      });
    }
    var els = [];
    var lines = [];
    var used = 0;
    // elements that look identical ("장바구니 담기" x N) get their surrounding text as a disambiguating context
    var seen = {};
    for (var d = 0; d < items.length; d++) {
      if (items[d].el) {
        items[d].desc = describe(items[d].el);
        seen[items[d].desc] = (seen[items[d].desc] || 0) + 1;
      }
    }
    for (var i = 0; i < items.length; i++) {
      var it = items[i];
      if (it.el) {
        if (els.length >= max) {
          truncated = true;
          continue;
        }
        els.push(it.el);
        var desc = it.desc;
        if (seen[desc] > 1) {
          var own = elText(it.el);
          var ctx = contextText(it.el);
          if (own && ctx.indexOf(own) >= 0) ctx = norm(ctx.replace(own, ''));
          if (ctx) desc = desc.slice(0, -3) + attr('context', ctx, 70) + ' />';
        }
        lines.push('[' + els.length + '] ' + desc);
      } else if (used < textBudget) {
        used += it.text.length;
        lines.push((it.heading ? 'heading: "' : 'text: "') + it.text.replace(/"/g, "'") + '"');
      }
    }
    J.distilled = els;
    return {
      url: location.href,
      title: document.title,
      scrollY: Math.round(window.scrollY),
      scrollHeight: document.documentElement.scrollHeight,
      viewportHeight: window.innerHeight,
      count: els.length,
      truncated: truncated,
      text: lines.join('\n'),
    };
  }

  function pick(index) {
    var el = J.distilled[index - 1];
    if (!el || !el.isConnected) return { status: 'notfound' };
    if (!visible(el)) return { status: 'hidden' };
    if (!enabled(el)) return { status: 'disabled' };
    J.last = el;
    var p = pointOf(el);
    return { status: 'ok', method: 'index', x: p.x, y: p.y, fingerprint: fingerprint(el), editable: isEditable(el), tag: el.tagName.toLowerCase(),
      password: el.tagName === 'INPUT' && (el.getAttribute('type') || '').toLowerCase() === 'password' };
  }

  // ---------- Set-of-Mark overlay ----------
  function somDraw() {
    somClear();
    var box = document.createElement('div');
    box.id = '__jm_som';
    box.style.cssText = 'position:fixed;left:0;top:0;width:0;height:0;z-index:2147483647;pointer-events:none;';
    var n = 0;
    for (var i = 0; i < J.distilled.length; i++) {
      var el = J.distilled[i];
      if (!el.isConnected) continue;
      var r = el.getBoundingClientRect();
      if (r.bottom < 0 || r.top > window.innerHeight || r.right < 0 || r.left > window.innerWidth || r.width < 1) continue;
      var b = document.createElement('div');
      b.style.cssText =
        'position:fixed;box-sizing:border-box;border:2px solid #ff0000;pointer-events:none;left:' +
        r.left + 'px;top:' + r.top + 'px;width:' + r.width + 'px;height:' + r.height + 'px;';
      var l = document.createElement('div');
      l.textContent = '[' + (i + 1) + ']';
      l.style.cssText =
        'position:fixed;background:#ff0000;color:#fff;font:bold 11px/13px Arial,sans-serif;padding:0 2px;pointer-events:none;left:' +
        Math.max(0, r.left) + 'px;top:' + Math.max(0, r.top - 13) + 'px;';
      box.appendChild(b);
      box.appendChild(l);
      n++;
    }
    document.documentElement.appendChild(box);
    return n;
  }
  function somClear() {
    var o = document.getElementById('__jm_som');
    if (o) o.remove();
    return true;
  }

  // ---------- helpers used by the executor ----------
  function prepareType() {
    var el = J.last;
    if (!el) return false;
    try {
      el.focus();
      if (el.select) el.select();
      else if (el.isContentEditable) {
        var range = document.createRange();
        range.selectNodeContents(el);
        var sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
      }
    } catch (e) {}
    return true;
  }
  function selectValue(v) {
    var el = J.last;
    if (!el || el.tagName !== 'SELECT') return false;
    var found = false;
    for (var i = 0; i < el.options.length; i++) {
      if (el.options[i].value === v || norm(el.options[i].text) === norm(v)) {
        el.selectedIndex = i;
        found = true;
        break;
      }
    }
    if (!found) return false;
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
    return true;
  }
  function pageText(n) {
    var t = document.body ? document.body.innerText || '' : '';
    t = t.replace(/[ \t]+/g, ' ').replace(/\n\s*\n+/g, '\n');
    return t.length > n ? t.slice(0, n) : t;
  }

  // ---------- login form detection (credential vault) ----------
  // Handles one-page forms (ID + password) and two-step logins (ID first, password on the next screen).
  var LOGIN_RE = /log[\s_-]?in|sign[\s_-]?in|sso|로그인|ログイン|登录|登入/i;
  var LOGIN_HOST_RE = /^(accounts?|auth|id|nid|sso|login|signin)\./i;
  var ID_RE = /user|login|e-?mail|account|identifier|(^|[^a-z])u?id($|[^a-z])|아이디|이메일|계정|휴대폰|전화번호/i;
  var NEXT_RE = /다음|계속|확인|로그인|next|continue|log\s?in|sign\s?in|submit/i;
  function inputType(el) {
    return (el.getAttribute('type') || 'text').toLowerCase();
  }
  function idLike(el) {
    if (!el || el.tagName !== 'INPUT') return false;
    var ty = inputType(el);
    if (['text', 'email', 'tel'].indexOf(ty) < 0) return false;
    if (ty === 'email' || /username|email/.test((el.getAttribute('autocomplete') || '').toLowerCase())) return true;
    var s = [el.getAttribute('name'), el.id, el.getAttribute('placeholder'), el.getAttribute('aria-label'), axName(el)].join(' ');
    return ID_RE.test(s);
  }
  // the form, or (for form-less SPA markup) a few ancestors up
  function loginScope(el) {
    var form = el.closest('form');
    if (form) return form;
    var s = el;
    for (var i = 0; i < 4 && s.parentElement && s.parentElement !== document.body && s.parentElement !== document.documentElement; i++) s = s.parentElement;
    return s;
  }
  function loginContext(el, scope) {
    if (/username/.test((el.getAttribute('autocomplete') || '').toLowerCase())) return true;
    if (LOGIN_RE.test(location.pathname) || LOGIN_HOST_RE.test(location.hostname) || LOGIN_RE.test(document.title)) return true;
    return LOGIN_RE.test((scope.innerText || '').slice(0, 400));
  }
  function loginFields() {
    var inputs = [];
    walk(document, function (el) {
      if (el.tagName === 'INPUT' && visible(el) && enabled(el)) inputs.push(el);
    });
    var i, k;
    for (i = 0; i < inputs.length; i++) {
      if (inputType(inputs[i]) === 'password') {
        var pass = inputs[i];
        var user = null;
        var form = pass.closest('form');
        for (k = i - 1; k >= 0; k--) {
          if (['text', 'email', 'tel', 'number', 'search'].indexOf(inputType(inputs[k])) < 0 || inputs[k].readOnly) continue;
          if (form && inputs[k].closest('form') !== form) continue;
          user = inputs[k];
          break;
        }
        return { pass: pass, user: user };
      }
    }
    // no password field: step 1 of a two-step login shows only the ID field
    for (i = 0; i < inputs.length; i++) {
      var el = inputs[i];
      if (el.readOnly || !idLike(el)) continue;
      var scope = loginScope(el);
      var texts = 0;
      for (k = 0; k < inputs.length; k++) {
        if (['text', 'email', 'tel', 'number'].indexOf(inputType(inputs[k])) >= 0 && scope.contains(inputs[k])) texts++;
      }
      if (texts > 2 || !loginContext(el, scope)) continue; // sign-up / newsletter forms are not logins
      return { pass: null, user: el };
    }
    return { pass: null, user: null };
  }
  function loginForm() {
    var f = loginFields();
    return {
      found: !!(f.pass || f.user),
      stage: f.pass ? (f.user ? 'full' : 'pass') : f.user ? 'id' : 'none',
      hasUser: !!f.user,
      host: location.hostname,
      userValue: f.user ? f.user.value : '',
    };
  }
  function loginField(which) {
    var f = loginFields();
    var el = which === 'user' ? f.user : f.pass;
    if (!el) return { status: 'notfound' };
    J.last = el;
    var p = pointOf(el);
    return { status: 'ok', x: p.x, y: p.y, fingerprint: fingerprint(el) };
  }
  // the "next" / "log in" button that belongs to a login field (used when Enter does not submit)
  function loginSubmit(which) {
    var f = loginFields();
    var field = which === 'user' ? f.user : f.pass;
    if (!field) return { status: 'notfound' };
    var scope = field.closest('form') || field.parentElement;
    for (var depth = 0; scope && depth < 7; depth++) {
      var best = null;
      var bestScore = 0;
      walk(scope, function (el) {
        var t = el.tagName;
        var ty = (el.getAttribute('type') || '').toLowerCase();
        var isBtn = t === 'BUTTON' || (t === 'INPUT' && (ty === 'submit' || ty === 'button')) || el.getAttribute('role') === 'button';
        if (!isBtn || !visible(el) || !enabled(el)) return;
        var label = t === 'INPUT' ? el.value || '' : axName(el) || elText(el);
        var score = (NEXT_RE.test(label) ? 3 : 0) + (ty === 'submit' || (t === 'BUTTON' && !ty && el.closest('form')) ? 2 : 0);
        if (score > bestScore) {
          best = el;
          bestScore = score;
        }
      });
      if (best) {
        J.last = best;
        var p = pointOf(best);
        return { status: 'ok', x: p.x, y: p.y, fingerprint: fingerprint(best) };
      }
      if (scope.tagName === 'FORM' || scope === document.body) break;
      scope = scope.parentElement;
    }
    return { status: 'notfound' };
  }

  var J = {
    loginForm: loginForm,
    loginField: loginField,
    loginSubmit: loginSubmit,
    distilled: [],
    last: null,
    resolve: resolve,
    distill: distill,
    pick: pick,
    fingerprint: fingerprint,
    fingerprintLast: function () {
      return J.last ? fingerprint(J.last) : null;
    },
    somDraw: somDraw,
    somClear: somClear,
    prepareType: prepareType,
    selectValue: selectValue,
    pageText: pageText,
  };
  Object.defineProperty(window, '__jm', { value: J, configurable: true, enumerable: false });

  // ---------- Module A: recorder ----------
  function emit(o) {
    try {
      if (typeof window.__jm_emit === 'function') {
        o.url = location.href;
        window.__jm_emit(JSON.stringify(o));
      }
    } catch (e) {}
  }
  var pending = null; // {el, fp, value}
  function flush() {
    if (!pending) return;
    var p = pending;
    pending = null;
    emit({ kind: 'type', fingerprint: p.fp, value: p.value, sensitive: p.sensitive, idField: p.idField });
  }
  function target(e) {
    var path = e.composedPath ? e.composedPath() : [];
    var t = path.length ? path[0] : e.target;
    if (t && t.nodeType === 3) t = t.parentElement;
    return t && t.nodeType === 1 ? t : null;
  }
  var lastUserScrollIntent = 0;

  window.addEventListener(
    'click',
    function (e) {
      if (!e.isTrusted) return;
      var t = target(e);
      if (!t) return;
      // detail===0: synthesized click (label -> input forwarding, keyboard activation). The causing event is recorded instead.
      if (e.detail === 0) return;
      var el = (t.closest && t.closest(REC_SEL)) || t;
      if (el.tagName === 'SELECT' || el.tagName === 'OPTION') return;
      if (el === document.documentElement || el === document.body) return;
      if (pending && pending.el !== el) flush();
      emit({ kind: 'click', fingerprint: fingerprint(el) });
    },
    true
  );
  window.addEventListener(
    'input',
    function (e) {
      if (!e.isTrusted) return;
      var t = target(e);
      if (!t || !isEditable(t)) return;
      if (pending && pending.el !== t) flush();
      var value = t.isContentEditable ? t.innerText : t.value;
      pending = {
        el: t,
        fp: pending && pending.el === t ? pending.fp : fingerprint(t),
        value: value,
        sensitive: (t.getAttribute('type') || '').toLowerCase() === 'password',
        idField: idLike(t),
      };
    },
    true
  );
  window.addEventListener(
    'change',
    function (e) {
      if (!e.isTrusted) return;
      var t = target(e);
      if (!t) return;
      if (t.tagName === 'SELECT') {
        flush();
        var o = t.options[t.selectedIndex];
        emit({ kind: 'select', fingerprint: fingerprint(t), value: t.value, label: o ? norm(o.text) : '' });
      } else if (pending && pending.el === t) flush();
    },
    true
  );
  window.addEventListener('focusout', function () { flush(); }, true);
  window.addEventListener('pagehide', function () { flush(); }, true);
  window.addEventListener(
    'keydown',
    function (e) {
      if (!e.isTrusted) return;
      lastUserScrollIntent = Date.now();
      var k = e.key;
      if (k === 'Shift' || k === 'Control' || k === 'Alt' || k === 'Meta' || k === 'Process' || e.isComposing) return;
      var t = target(e);
      if (t && t.tagName === 'SELECT') return; // the resulting change event is recorded instead
      var editable = isEditable(t);
      var combo = e.ctrlKey || e.altKey || e.metaKey;
      var record = false;
      if (combo) record = !editable;
      else if (k === 'Enter') record = !(editable && t.tagName === 'TEXTAREA' ) && !(editable && t.isContentEditable && e.shiftKey);
      else if (k === 'Tab' || k === 'Escape' || k === 'ArrowDown' || k === 'ArrowUp' || k === 'PageDown' || k === 'PageUp' || /^F\d+$/.test(k)) record = true;
      else if (k === 'ArrowLeft' || k === 'ArrowRight' || k === 'Home' || k === 'End' || k === ' ' || k === 'Backspace' || k === 'Delete') record = !editable;
      if (!record) return;
      flush();
      var mod = (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.metaKey ? 4 : 0) | (e.shiftKey ? 8 : 0);
      emit({ kind: 'key', key: k, code: e.code, modifiers: mod });
    },
    true
  );
  ['wheel', 'touchmove', 'mousedown'].forEach(function (n) {
    window.addEventListener(n, function () { lastUserScrollIntent = Date.now(); }, { capture: true, passive: true });
  });
  var scrollTimer = null;
  window.addEventListener(
    'scroll',
    function (e) {
      if (e.target !== document) return;
      if (Date.now() - lastUserScrollIntent > 1500) return;
      clearTimeout(scrollTimer);
      scrollTimer = setTimeout(function () {
        emit({ kind: 'scroll', scrollX: Math.round(window.scrollX), scrollY: Math.round(window.scrollY) });
      }, 500);
    },
    true
  );
})();
