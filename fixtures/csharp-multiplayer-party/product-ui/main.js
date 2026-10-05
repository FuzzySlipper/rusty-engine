// The party panel: DOM only. It shows what the product projects and sends
// every action to the product as a party.ui intent. All product text,
// chat included, is set with textContent and never parsed as HTML.
const INTENT = 'party.ui';
const CONTRACT = 'party.ui.v1';
const FACING = ['north', 'east', 'south', 'west'];

export function mountProductUi(root, context) {
  context.ui?.setInteractionMode?.('interface');
  const panel = el('div', 'party-panel');
  panel.style.cssText = 'position:absolute;left:8px;top:8px;bottom:8px;width:380px;overflow:auto;padding:10px;background:rgba(10,12,18,.82);color:#eee;font:13px system-ui,sans-serif;border-radius:6px';
  root.append(panel);
  const send = (data) => context.intents?.claim(INTENT, { kind: 'product-payload', contract: CONTRACT, data });

  // Inputs keep their values across projections.
  const name = input('Your name', 'name');
  const relay = select('relay', [['n0', 'n0 public relay (development)'], ['', 'Direct only (LAN)'], ['custom', 'Custom relay URL']]);
  const relayUrl = input('https://relay.example.org/', 'relay-url');
  const relayToken = input('Relay access token (if it needs one)', 'relay-token');
  const relayOnly = checkbox('relay-only', 'Relay only (hide addresses)');
  const invitation = textarea('Paste an invitation', 'invitation');
  const chatText = input('Say something', 'chat-text');
  chatText.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && chatText.value.trim() !== '') { send({ action: 'chat', text: chatText.value }); chatText.value = ''; }
  });
  const relayChoice = () => (relay.value === 'custom' ? relayUrl.value.trim() : relay.value);

  const sessionBox = el('section', 'session');
  const membersBox = el('section', 'members');
  const partyBox = el('section', 'party');
  const chatBox = el('section', 'chat');
  const noticeLine = el('p', 'notice');
  noticeLine.style.color = '#ffcf66';
  panel.append(noticeLine, sessionBox, membersBox, partyBox, chatBox);

  // A section is rebuilt only when what it shows changed, so typing in an
  // input is not interrupted by the periodic projection.
  const shown = new Map();
  const changed = (section, value) => {
    const key = JSON.stringify(value);
    if (shown.get(section) === key) return false;
    shown.set(section, key);
    return true;
  };
  const render = (value) => {
    noticeLine.textContent = value.notice || '';
    if (changed('session', [value.session, value.name])) renderSession(value);
    if (changed('members', [value.members, value.party?.seats, value.party?.leader])) renderMembers(value);
    if (changed('party', [value.party, value.session.member])) renderParty(value);
    if (changed('chat', [value.chat, value.party?.seats])) renderChat(value);
  };

  function renderSession(value) {
    sessionBox.replaceChildren(heading('Session'));
    const session = value.session;
    if (session.state === 'none') {
      if (!name.value && value.name) name.value = value.name;
      sessionBox.append(
        name, row(relay, relayUrl), relayToken, relayOnly,
        button('Host a party', 'host', () => send({ action: 'host', name: name.value, relay: relayChoice(), relayToken: relayToken.value.trim(), relayOnly: relayOnly.checked })),
        invitation,
        button('Join', 'join', () => send({ action: 'join', name: name.value, invitation: invitation.value, relayOnly: relayOnly.checked })),
      );
      return;
    }
    sessionBox.append(text(`${value.name}: ${session.role} · ${session.state}${session.member ? ` · member ${session.member}` : ''}`, 'status'));
    if (session.ended) sessionBox.append(text(`Ended: ${session.ended}`, 'ended'));
    if (session.invitation) {
      const shown = textarea('', 'invitation-out');
      shown.value = session.invitation;
      shown.readOnly = true;
      sessionBox.append(text('Invitation (share it in any chat):'), shown,
        button('Copy invitation', 'copy', () => { shown.select(); navigator.clipboard?.writeText(session.invitation).catch(() => {}); }));
    }
    sessionBox.append(button(session.state === 'Ended' ? 'Close' : 'Leave', 'leave', () => send({ action: 'leave' })));
  }

  function renderMembers(value) {
    membersBox.replaceChildren();
    if (!value.members) return;
    const seats = new Map((value.party?.seats ?? []).map((seat) => [seat.member, seat]));
    const leader = value.party?.leader;
    membersBox.append(heading('Members'));
    for (const member of value.members) {
      const seat = seats.get(member.member);
      const tags = [member.host ? 'host' : '', member.member === leader ? 'leader' : '', member.local ? 'you' : '', member.awaitingView ? 'joining' : ''].filter(Boolean).join(', ');
      // Guests connect only to the host; other guests are reached through it.
      const link = member.local ? '' : !member.connected ? ' · away' : member.path === 'None' ? ' · via host' : ` · ${member.path} ${member.rttMs} ms`;
      const line = text(`#${member.member} ${seat?.name ?? member.key}${seat?.character ? ` (${seat.character})` : ''}${tags ? ` [${tags}]` : ''}${link}`, 'member');
      line.dataset.member = String(member.member);
      membersBox.append(line);
      if (leader === value.session.member && member.member !== leader && member.connected && seat)
        membersBox.append(button(`Make ${seat.name} leader`, `lead-${member.member}`, () => send({ action: 'lead', member: member.member })));
    }
  }

  function renderParty(value) {
    partyBox.replaceChildren();
    const party = value.party;
    if (!party) return;
    const me = value.session.member;
    const leading = party.leader === me;
    const mySeat = party.seats.find((seat) => seat.member === me);
    partyBox.append(heading('Party'), text(`At (${party.x}, ${party.z}) facing ${FACING[party.facing]} · ${party.phase} · revision ${party.revision}`, 'position'));
    if (!mySeat?.character) {
      const taken = new Set(party.seats.map((seat) => seat.character));
      partyBox.append(row(...value.characters.filter((c) => !taken.has(c)).map((c) => button(`Play ${c}`, `claim-${c}`, () => send({ action: 'claim', character: c })))));
    }
    if (party.phase === 'explore') {
      if (leading) {
        partyBox.append(row(
          button('Turn left', 'move-left', () => send({ action: 'move', move: 'left' })),
          button('Forward', 'move-forward', () => send({ action: 'move', move: 'forward' })),
          button('Turn right', 'move-right', () => send({ action: 'move', move: 'right' })),
          button('Back', 'move-back', () => send({ action: 'move', move: 'back' })),
        ));
      } else {
        partyBox.append(text('The leader moves the party.'));
      }
    }
    if (party.phase === 'vote' || party.phase === 'combat') {
      const verb = party.phase === 'vote' ? 'vote' : 'act';
      partyBox.append(text(party.prompt, 'prompt'));
      const chosen = party.chosen.includes(me);
      if (!chosen && (party.phase === 'vote' || mySeat?.character))
        partyBox.append(row(...party.options.map((option, index) => button(option, `${verb}-${index}`, () => send({ action: verb, choice: index, phase: party.phaseId })))));
      const waiting = party.waiting.map((member) => {
        const seat = party.seats.find((candidate) => candidate.member === member);
        return seat ? `${seat.name}${seat.present ? '' : ' (away)'}` : `#${member}`;
      });
      partyBox.append(text(waiting.length ? `Waiting for: ${waiting.join(', ')}` : 'Everyone has chosen.', 'waiting'));
      if (leading && waiting.length) partyBox.append(button('Decide without them', 'resolve', () => send({ action: 'resolve', phase: party.phaseId })));
    }
    if (party.phase === 'result' && leading) partyBox.append(button('Continue', 'continue', () => send({ action: 'continue' })));
    const log = el('ol', 'log');
    for (const line of party.log) log.append(text(line, 'log-line', 'li'));
    partyBox.append(log);
  }

  function renderChat(value) {
    chatBox.replaceChildren();
    if (!value.chat) return;
    const names = new Map((value.party?.seats ?? []).map((seat) => [seat.member, seat.name]));
    chatBox.append(heading('Chat'));
    const lines = el('div', 'chat-lines');
    lines.style.cssText = 'max-height:180px;overflow:auto';
    for (const line of value.chat) {
      const state = line.state === 'Delivered' ? '' : ` (${line.state.toLowerCase()})`;
      lines.append(text(`${names.get(line.member) ?? `#${line.member}`}: ${line.text}${state}`, 'chat-line'));
    }
    chatBox.append(lines, chatText, button('Send', 'chat-send', () => {
      if (chatText.value.trim() === '') return;
      send({ action: 'chat', text: chatText.value });
      chatText.value = '';
    }));
    lines.scrollTop = lines.scrollHeight;
  }

  const unsubscribe = context.projection?.subscribe((projection) => {
    if (projection?.contract === 'party.panel.v1' && projection.value?.session) render(projection.value);
  }) ?? (() => {});
  return { dispose() { unsubscribe(); panel.remove(); } };
}

function el(tag, name) {
  const node = document.createElement(tag);
  if (name) node.dataset.party = name;
  return node;
}

function text(value, name, tag = 'div') {
  const node = el(tag, name);
  node.textContent = value;
  return node;
}

function heading(value) {
  const node = text(value, undefined, 'h3');
  node.style.cssText = 'margin:10px 0 4px;font-size:14px';
  return node;
}

function button(label, name, onClick) {
  const node = el('button', name);
  node.textContent = label;
  node.style.margin = '2px';
  node.addEventListener('click', onClick);
  return node;
}

function row(...children) {
  const node = el('div');
  node.style.cssText = 'display:flex;flex-wrap:wrap;gap:4px;margin:4px 0';
  node.append(...children);
  return node;
}

function input(placeholder, name) {
  const node = el('input', name);
  node.placeholder = placeholder;
  node.style.cssText = 'width:100%;box-sizing:border-box;margin:2px 0';
  return node;
}

function textarea(placeholder, name) {
  const node = el('textarea', name);
  node.placeholder = placeholder;
  node.rows = 3;
  node.style.cssText = 'width:100%;box-sizing:border-box;margin:2px 0;font:11px ui-monospace,monospace';
  return node;
}

function select(name, options) {
  const node = el('select', name);
  for (const [value, label] of options) {
    const option = document.createElement('option');
    option.value = value;
    option.textContent = label;
    node.append(option);
  }
  return node;
}

function checkbox(name, label) {
  const box = el('input', name);
  box.type = 'checkbox';
  const wrap = el('label');
  wrap.append(box, document.createTextNode(` ${label}`));
  Object.defineProperty(wrap, 'checked', { get: () => box.checked });
  return wrap;
}
