// Platform editing only. Rust owns committed text, selection and composition.
const encoder = new TextEncoder();
export function utf16ToByte(text, offset) {
  let units = 0, bytes = 0;
  for (const character of text) {
    if (units === offset) return bytes;
    units += character.length; bytes += encoder.encode(character).length;
    if (units > offset) return null; // Never bisect a surrogate pair.
  }
  return units === offset ? bytes : null;
}
export function byteToUtf16(text, offset) {
  let units = 0, bytes = 0;
  for (const character of text) {
    if (bytes === offset) return units;
    bytes += encoder.encode(character).length; units += character.length;
    if (bytes > offset) return null;
  }
  return bytes === offset ? units : null;
}
export function wireTextProxy(el, {listen, send, node, clipboard, invalidate}) {
  let composing = false, cancelled = false, lastComposition = false, restoring = false;
  let draft='',compositionStart=0,compositionBase='';
  function preedit() {
    const start=privateInput()?0:compositionStart;
    const a=utf16ToByte(draft,Math.max(0,Math.min(draft.length,(el.selectionStart || 0)-start)));
    const b=utf16ToByte(draft,Math.max(0,Math.min(draft.length,(el.selectionEnd || 0)-start)));
    if(a!==null && b!==null) send({op:'preedit',id:node().id,text:draft,anchor:a,caret:b});
  }
  const privateInput = () => node().password || node().code;
  function selection() {
    const a = utf16ToByte(el.value, el.selectionStart || 0), b = utf16ToByte(el.value, el.selectionEnd || 0);
    return a === null || b === null ? null : el.selectionDirection === 'backward' ? [b,a] : [a,b];
  }
  function select() {
    if (restoring || composing || privateInput() || document.activeElement !== el) return;
    const range = selection();
    if (range) send({op:'select',id:node().id,anchor:range[0],caret:range[1]});
  }
  function value() {
    if (encoder.encode(el.value).length > 48000) { el.value = node().value || ''; return; }
    const range = selection();
    if (range) send({op:'editor_value',id:node().id,text:el.value,anchor:range[0],caret:range[1]});
  }
  function cancel() {
    if (composing) send({op:'composition',id:node().id,phase:'cancel',text:''});
    composing = false; cancelled = true;draft='';compositionBase='';
    if (privateInput()) el.value = '';
  }
  listen(el,'compositionstart',()=>{
    if (el.readOnly) return;
    select(); compositionStart=el.selectionStart || 0;draft='';compositionBase=privateInput()?'':el.value;
    composing = true; cancelled = false; lastComposition = false;
  });
  listen(el,'compositionupdate',event=>{
    if (composing && !cancelled) {
      draft=event.data || '';
      send({op:'preedit',id:node().id,text:draft,anchor:encoder.encode(draft).length,caret:encoder.encode(draft).length});
    }
  });
  listen(el,'compositionend',event=>{
    if (!composing || cancelled || el.readOnly || document.activeElement !== el) { cancel(); return; }
    composing = false; lastComposition = true;draft='';compositionBase='';
    if (!event.data) {
      send({op:'composition',id:node().id,phase:'cancel',text:''});
      el.value=privateInput() ? '' : (node().value || '');
      return;
    }
    if (privateInput()) {
      send({op:'composition',id:node().id,phase:'update',text:event.data});
      send({op:'composition',id:node().id,phase:'commit',text:event.data});
      el.value = '';
    } else {
      send({op:'composition',id:node().id,phase:'cancel',text:''});
      value();
    }
  });
  listen(el,'beforeinput',event=>{
    if(document.activeElement!==el || !privateInput() || composing || event.isComposing || el.readOnly || !event.cancelable) return;
    const key={deleteContentBackward:'Backspace',deleteContentForward:'Delete',insertLineBreak:'Enter',insertParagraph:'Enter',historyUndo:'Undo',historyRedo:'Redo'}[event.inputType];
    if(key) {event.preventDefault();send({op:'edit_key',id:node().id,key});}
  });
  listen(el,'input',event=>{
    if (el.readOnly || document.activeElement!==el) {if(privateInput())el.value='';return;}
    if(cancelled && /Composition/.test(event.inputType || '')) {el.value=privateInput()?'':(node().value || '');return;}
    if(composing || event.isComposing) {if(composing)preedit();return;}
    if (lastComposition && /Composition/.test(event.inputType || '')) {
      if (privateInput()) el.value = '';
      lastComposition = false; return;
    }
    lastComposition = false;
    if (privateInput()) {
      const text = el.value; el.value = '';
      if (text) send({op:'insert',id:node().id,text});
    } else value();
  });
  listen(el,'select',select);
  listen(el,'keyup',select);
  listen(el,'pointerup',select);
  listen(el,'blur',cancel);
  listen(el,'keydown',event=>{
    if (composing || event.isComposing || event.keyCode === 229) return;
    const primary = event.ctrlKey || event.metaKey;
    let key = event.key;
    if (primary && key.toLowerCase() === 'z') key = event.shiftKey ? 'Redo' : 'Undo';
    if (primary && key.toLowerCase() === 'a') key = 'SelectAll';
    if (privateInput() && ['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End','PageUp','PageDown','Backspace','Delete','Enter','Tab','Undo','Redo','SelectAll','Escape'].includes(key)) {
      // Tab remains normal focus navigation for passwords, indentation for code.
      if (key === 'Tab' && !node().code) return;
      event.preventDefault(); event.stopPropagation();
      send({op:'edit_key',id:node().id,key,shift:event.shiftKey,control:primary,alt:event.altKey,meta:event.metaKey});
      if (key === 'Escape') cancel();
    } else if (key === 'Enter' && !node().multiline) {
      event.preventDefault(); send({op:'edit_key',id:node().id,key});
    } else if (key === 'Escape') { cancel(); el.blur(); }
  });
  for (const kind of ['copy','cut']) listen(el,kind,event=>{
    if (node().password) { event.preventDefault(); return; }
    if (node().code) {
      event.preventDefault();
      if(document.activeElement!==el) return;
      const text = clipboard(kind === 'cut');
      if (text !== undefined && text !== null) event.clipboardData?.setData('text/plain',text);
      invalidate();
    }
  });
  listen(el,'paste',event=>{
    if (!privateInput() || composing) return;
    if(document.activeElement!==el) {event.preventDefault();return;}
    event.preventDefault();
    if (!el.readOnly) send({op:'insert',id:node().id,text:event.clipboardData?.getData('text/plain') || '',paste:true});
  });
  return {
    cancel,
    sync(n) {
      if(composing && (n.readOnly || !n.enabled || !n.focused || (!privateInput() && (n.value || '')!==compositionBase)))cancel();
      if (privateInput()) { if (!composing) el.value = ''; return; }
      if (composing) return; // Native IME owns the transient DOM draft.
      const next = n.value || '';
      restoring = true;
      if (el.value !== next) el.value = next;
      if (n.selection && document.activeElement === el) {
        const [anchor,caret]=n.selection;
        const a=byteToUtf16(next,Math.min(anchor,caret)),b=byteToUtf16(next,Math.max(anchor,caret));
        if (a!==null && b!==null && (el.selectionStart!==a || el.selectionEnd!==b || el.selectionDirection!==(anchor>caret?'backward':'forward'))) {
          el.setSelectionRange(a,b,anchor>caret?'backward':'forward');
        }
      }
      restoring = false;
    },
  };
}
