// Runs in an isolated script world (page JavaScript is disabled).
// Adds syntax highlighting and copy buttons, and lets the app swap in new content in place.
(() => {
  hljs.configure({ ignoreUnescapedHTML: true });

  function enhance(root) {
    root.querySelectorAll('pre > code').forEach((code) => {
      const language = [...code.classList].find((c) => c.startsWith('language-'))?.slice(9);
      if (language && hljs.getLanguage(language)) {
        hljs.highlightElement(code);
      }

      const pre = code.parentElement;
      const wrapper = document.createElement('div');
      wrapper.className = 'code-block';
      pre.replaceWith(wrapper);
      wrapper.appendChild(pre);

      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'copy-button';
      button.textContent = 'Copy';
      button.addEventListener('click', () => {
        window.webkit.messageHandlers.copyCode.postMessage(code.textContent);
        button.textContent = 'Copied';
        button.classList.add('copied');
        clearTimeout(button.resetTimer);
        button.resetTimer = setTimeout(() => {
          button.textContent = 'Copy';
          button.classList.remove('copied');
        }, 1500);
      });
      wrapper.appendChild(button);
    });
  }

  const article = () => document.querySelector('article.markdown-body');

  window.mdviewer = {
    update(html) {
      const root = article();
      root.innerHTML = html;
      enhance(root);
    },
  };

  enhance(article());
})();
