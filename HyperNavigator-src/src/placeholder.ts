// Local page shown in the guest view when no app is loaded. The Host passes state in the URL
// fragment; it is rendered with textContent only.
const params = new URLSearchParams(window.location.hash.slice(1));

if (params.get('state') === 'error') {
  const message = (params.get('message') ?? 'Unknown error').slice(0, 600);
  document.getElementById('ready')!.hidden = true;
  document.getElementById('error')!.hidden = false;
  document.getElementById('error-message')!.textContent = message;
}
