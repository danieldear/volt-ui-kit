#!/usr/bin/env node
/* Maintainer-only generator: Node 22+, marked 17.0.5. No JS ships to browsers.
 * Usage: NODE_PATH=<directory containing marked> node scripts/generate_docs.cjs EXPORT_DIR
 * EXPORT_DIR comes from catalog --export-docs NEW_DIRECTORY.
 */
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { marked } = require('marked');
const ROOT = path.resolve(__dirname, '..');
const DOCS = path.join(ROOT, 'docs');
const source = process.argv[2];
if (!source) throw new Error('Pass the Catalog export directory');
const catalog = JSON.parse(fs.readFileSync(path.join(source, 'catalog.json'), 'utf8'));
const escape = s => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const safeSlug = s => /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(s);
const slugify = s => s.toLowerCase().replace(/<[^>]+>/g, '').replace(/[^\p{L}\p{N}_\s-]/gu, '').trim().replace(/\s/g, '-');
const guideDefs = [
 ['getting-started','Getting started','Install the kit, create a project and open your first native window.'],
 ['building-uis','Building interfaces','State, typed callbacks, layout, layers and responsive composition.'],
 ['components','Component recipes','A practical overview of the Rust component builders.'],
 ['styling-and-themes','Styling and themes','Semantic colors, type, custom fonts, images, effects and motion.'],
 ['windows-and-rendering','Windows and rendering','Window chrome, GPU drawing, transparency and desktop blur.'],
 ['app-services','Application services','Background work, commands, files, menus, notifications and packaging.'],
 ['json-ui','Optional JSON frontend','Data-only declarations with behavior owned by your Rust application.'],
 ['architecture','Architecture','The frame pipeline, ownership, accessibility and resource boundaries.'],
];
const files = [];
function write(name, content) {
 const target = path.join(DOCS, name); fs.mkdirSync(path.dirname(target), {recursive:true});fs.writeFileSync(target, content); files.push(name);
}
function localLink(href, guide=false) {
 const parts = href.split('#');
 const base = parts[0].replace(/^\.\//,'');
 if (/^(?:https:|mailto:|#)/.test(href)) return href;
 if (!base) return href;
 const clean = base.replace(/^\.\.\//,'');
 const found = guideDefs.find(([slug]) => `${slug}.md` === clean);
 if (found) return `../guides/${found[0]}.html${parts.length>1?'#'+parts.slice(1).join('#'):''}`;
 if (/^[a-zA-Z0-9_./-]+(?:#[\w-]+)?$/.test(href) && !href.startsWith('/')) {
  // Source links outside the published guide set stay on GitHub.
  const repoPath = path.posix.normalize((guide?'docs/':'')+href).replace(/^\.\.\//,'');
  return `https://github.com/danieldear/volt-ui-kit/blob/main/${repoPath}`;
 }
 throw new Error(`Unsupported guide link: ${href}`);
}
const csp = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; base-uri 'none'; form-action 'none'; object-src 'none'";
const groups = ['Getting started','Components','Patterns'];
function navigation(active) {
 return `<div class="reference-nav"><a class="nav-overview${active==='all'?' current':''}" href="../reference/index.html"${active==='all'?' aria-current="page"':''}>All components &amp; patterns</a><div class="nav-group"><p>Guides</p>${guideDefs.map(([slug,title])=>`<a href="../guides/${slug}.html"${active==='guide-'+slug?' aria-current="page"':''}>${escape(title)}</a>`).join('')}</div>${groups.map(group=>`<div class="nav-group"><p>${group==='Getting started'?'Foundations':group}</p>${catalog.filter(p=>p.group===group).map(p=>`<a href="../reference/${p.slug}.html"${active===p.slug?' aria-current="page"':''}>${escape(p.title)}</a>`).join('')}</div>`).join('')}<a class="nav-source" href="https://github.com/danieldear/volt-ui-kit">Source on GitHub ↗</a></div>`;
}
function shell({title,summary,active,body,toc=[]}) {
 return `<!doctype html>\n<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="description" content="${escape(summary)}"><meta name="color-scheme" content="light dark"><meta name="referrer" content="strict-origin-when-cross-origin"><meta http-equiv="Content-Security-Policy" content="${csp}"><title>${escape(title)} — Volt UI Kit</title><link rel="icon" href="../assets/favicon.svg" type="image/svg+xml"><link rel="preload" href="../assets/fonts/bricolage-grotesque.woff2" as="font" type="font/woff2" crossorigin><link rel="stylesheet" href="../assets/site.css"><link rel="stylesheet" href="../assets/reference.css"></head><body class="reference-page"><a class="skip" href="#main">Skip to content</a><header class="reference-header"><a class="reference-brand" href="../index.html"><img src="../assets/favicon.svg" width="30" height="30" alt="">Volt <span>UI Kit</span></a><nav aria-label="Main navigation"><a href="../reference/index.html">Components</a><a href="../guides/getting-started.html">Guides</a><a href="https://github.com/danieldear/volt-ui-kit">GitHub ↗</a></nav><span class="reference-version">0.5 / Pre-1.0</span></header><div class="reference-layout"><aside class="reference-sidebar" aria-label="Documentation navigation">${navigation(active)}</aside><details class="mobile-doc-nav"><summary>Browse documentation</summary>${navigation(active)}</details><main id="main" class="reference-main"><div class="reference-breadcrumb"><a href="../index.html">Docs</a><span aria-hidden="true">/</span>${escape(title)}</div>${body}<footer class="reference-footer">Built with Rust. Documented with the source.<a href="https://github.com/danieldear/volt-ui-kit/blob/main/docs/third-party.md">Licenses &amp; credits</a></footer></main><aside class="reference-toc" aria-label="On this page"><p>On this page</p>${toc.map(([id,label])=>`<a href="#${id}">${escape(label)}</a>`).join('')}<div class="toc-note">These are static docs. Run the native Catalog to try input, focus and animation.</div></aside></div></body></html>\n`;
}
function codeBlock(code, label='Rust declaration') {
 return `<div class="reference-code"><div>${escape(label)}<span>Source</span></div><pre tabindex="0" role="region" aria-label="${escape(label)}"><code>${escape(code)}</code></pre></div>`;
}
function preview(s, title, eager=false) {
 return `<figure class="component-capture"><a href="../images/components/${escape(s.image)}" aria-label="Open full ${escape(title)} preview"><img src="../images/components/${escape(s.image)}" width="${s.width}" height="${s.height}" alt="Toolkit-rendered ${escape(title)} preview" loading="${eager?'eager':'lazy'}"></a><figcaption>Rendered by Volt <span>Settled software snapshot · click to enlarge</span></figcaption></figure>`;
}
const totalSections = catalog.reduce((n,p)=>n+p.sections.length,0);
for (const p of catalog) {
 if (!safeSlug(p.slug)) throw new Error('Unsafe page slug');
 for (const s of p.sections) {
  if (!safeSlug(s.id) || !/^[a-z0-9-]+\.png$/.test(s.image)) throw new Error('Unsafe section or image name');
  const input=path.join(source,s.image); if(fs.lstatSync(input).isSymbolicLink())throw new Error('Linked image');
  const name=`images/components/${s.image}`;const target=path.join(DOCS,name);fs.mkdirSync(path.dirname(target),{recursive:true});fs.copyFileSync(input,target);files.push(name);
 }
 const previous=catalog[catalog.indexOf(p)-1],next=catalog[catalog.indexOf(p)+1];
 let body=`<div class="reference-title"><p>${escape(p.group==='Getting started'?'Foundation':p.group)}</p><h1>${escape(p.title)}</h1><div class="reference-description">${escape(p.summary)}</div></div><div class="reference-callout"><strong>Your state. Rust callbacks.</strong><p>The examples below come from the native Catalog. Preview images are rendered by the toolkit, not recreated in HTML. They do not demonstrate native input or desktop blur.</p></div>`;
 body+=p.sections.map((s,i)=>`<section class="reference-section" id="${s.id}"><div class="section-title"><h2>${escape(s.title)}</h2><span>${String(i+1).padStart(2,'0')} / ${String(p.sections.length).padStart(2,'0')}</span></div><p class="section-description">${escape(s.note)}</p>${preview(s,s.title,i===0)}<details class="usage-source"${i===0?' open':''}><summary>How to use <span>Rust source</span></summary>${codeBlock(s.code)}<p class="snippet-note">Catalog excerpt: names such as <code>app</code>, callbacks and helper functions belong to the example. <a href="https://github.com/danieldear/volt-ui-kit/tree/main/examples/catalog">View the complete source</a> or start with the <a href="../guides/getting-started.html#your-first-window">runnable app guide</a>.</p></details></section>`).join('');
 if(!p.sections.length)body+=`<section class="reference-section" id="first-window"><h2>Start with a native window</h2><p>Compose Rust builders in a view function, connect typed callbacks, and let the native host handle layout, drawing and input.</p><a class="button button-primary" href="../guides/getting-started.html">Build your first application</a></section>`;
 body+=`<section class="reference-section" id="native-catalog"><h2>Try it in the native Catalog</h2><p>From the repository root. Static snapshots cannot demonstrate editing, keyboard focus, scrolling or animation.</p>${codeBlock(`cargo run --release --locked \\\n    --features native-gpu,native-desktop \\\n    --example catalog -- --page ${p.slug}`,'Terminal')}</section>`;
 if(p.api.length)body+=`<section class="reference-section" id="api"><h2>API reference</h2><p>Properties and behavior documented by this Catalog page.</p><div class="table-scroll" tabindex="0" role="region" aria-label="${escape(p.title)} API"><table><thead><tr><th scope="col">API</th><th scope="col">Behavior</th></tr></thead><tbody>${p.api.map(([name,note])=>`<tr><th scope="row"><code>${escape(name)}</code></th><td>${escape(note)}</td></tr>`).join('')}</tbody></table></div></section>`;
 body+=`<nav class="reference-pagination" aria-label="Adjacent pages">${previous?`<a href="${previous.slug}.html"><small>Previous</small>${escape(previous.title)}</a>`:'<span></span>'}${next?`<a href="${next.slug}.html"><small>Next</small>${escape(next.title)}</a>`:''}</nav>`;
 write(`reference/${p.slug}.html`,shell({title:p.title,summary:p.summary,active:p.slug,body,toc:[...p.sections.map(s=>[s.id,s.title]),['native-catalog','Run the Catalog'],...(p.api.length?[['api','API reference']]:[])]}));
}
let listing=`<div class="reference-title index-title"><p>The component library</p><h1>The pieces.<br>Your interface.</h1><div class="reference-description">Explore the toolkit from its foundations to complete application patterns. Every example pairs a Volt-rendered preview with the Rust that defines it.</div><div class="library-stats"><span><strong>${catalog.length}</strong> Catalog pages</span><span><strong>${totalSections}</strong> Rendered examples</span><span><strong>Rust</strong> State &amp; callbacks</span></div></div><div class="index-jump">${groups.map(g=>`<a href="#group-${slugify(g)}">${g==='Getting started'?'Foundations':g}</a>`).join('')}</div>`;
for(const group of groups)listing+=`<section class="reference-section" id="group-${slugify(group)}"><h2>${group==='Getting started'?'Foundations':group}</h2><div class="component-directory">${catalog.filter(p=>p.group===group).map(p=>`<a href="${p.slug}.html" class="directory-item">${p.sections.length?`<div class="directory-preview"><img src="../images/components/${p.sections[0].image}" width="1000" height="${p.sections[0].height}" alt="${escape(p.title)} rendered preview" loading="lazy"></div>`:`<div class="directory-type" aria-hidden="true">Hello,<br>native.</div>`}<div class="directory-copy"><h3>${escape(p.title)} <span aria-hidden="true">↗</span></h3><p>${escape(p.summary)}</p><small>${p.sections.length} ${p.sections.length===1?'example':'examples'}</small></div></a>`).join('')}</div></section>`;
write('reference/index.html',shell({title:'Components',summary:'All Volt UI Kit components, source-backed examples and native application patterns.',active:'all',body:listing,toc:groups.map(g=>['group-'+slugify(g),g==='Getting started'?'Foundations':g])}));
for(const [slug,title,summary] of guideDefs){
 const toc=[], counts=new Map();
 const renderer=new marked.Renderer();
 renderer.html=token=>escape(token.text); // Repository Markdown cannot inject active HTML.
 renderer.heading=function(token){const text=this.parser.parseInline(token.tokens);const stem=slugify(token.text);const count=counts.get(stem)||0;counts.set(stem,count+1);const id=stem+(count?'-'+count:'');if(token.depth>1&&token.depth<4)toc.push([id,token.text.replace(/`/g,'')]);return `<h${token.depth} id="${id}">${text}</h${token.depth}>\n`;};
 renderer.link=function(token){return `<a href="${escape(localLink(token.href,true))}">${this.parser.parseInline(token.tokens)}</a>`;};
 renderer.image=token=>`<a href="${escape(localLink(token.href,true))}">${escape(token.text)}</a>`;
 renderer.code=token=>codeBlock(token.text.split('\n').filter(l=>!l.startsWith('# ')).map(l=>l.startsWith('## ')?l.slice(1):l).join('\n'),token.lang?.split(',')[0]||'Code');
 renderer.table=function(token){const row=(cells,tag)=>'<tr>'+cells.map(c=>`<${tag}${tag==='th'?' scope="col"':''}>${this.parser.parseInline(c.tokens)}</${tag}>`).join('')+'</tr>';return `<div class="table-scroll" tabindex="0" role="region" aria-label="Guide table"><table><thead>${row(token.header,'th')}</thead><tbody>${token.rows.map(r=>row(r,'td')).join('')}</tbody></table></div>`;};
 const markdown=fs.readFileSync(path.join(DOCS,slug+'.md'),'utf8').replace(/^# [^\n]+\n/,'');
 const content=marked.parse(markdown,{renderer,gfm:true});
 const body=`<div class="reference-title"><p>Guide</p><h1>${escape(title)}</h1><div class="reference-description">${escape(summary)}</div></div><article class="guide-article">${content}</article><div class="guide-source-link"><a href="https://github.com/danieldear/volt-ui-kit/blob/main/docs/${slug}.md">Edit this guide on GitHub ↗</a></div>`;
 write(`guides/${slug}.html`,shell({title,summary,active:'guide-'+slug,body,toc}));
}
write('assets/catalog-data.json',JSON.stringify(catalog,null,2)+'\n');
fs.writeFileSync(path.join(DOCS,'site-manifest.json'),JSON.stringify(files.sort(),null,2)+'\n');
console.log(`Generated ${catalog.length} Catalog pages, ${guideDefs.length} guides and ${totalSections} native-rendered previews.`);
