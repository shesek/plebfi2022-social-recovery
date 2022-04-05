let lib

import('../pkg/index.js').then(module => {
  lib = module
}).catch(console.error)

const $ = s => document.querySelector(s)

document.querySelector('form').addEventListener('submit', e => {
  e.preventDefault()
  let total_shares = +document.querySelector('#total-shares').value;
  let needed_shares = +document.querySelector('#needed-shares').value;
  let time_delay = document.querySelector('#time-delay').value;
  let label = document.querySelector('#label').value;
  let text = document.querySelector('#text').value;
  let result = lib.create_wallet(total_shares, needed_shares, time_delay)
  let params = result.params

  let label_seg = label ? `for '${label}' ` : ''

  $('#user-backup-hex').innerHTML = result.user_backup_hex
  $('#shares').innerHTML = result.shares.map((share, n) => `
    <h4>Recovery Share #${n+1} ${label_seg}(requires ${params.needed_shares}-of-${params.total_shares})</h4> 
    ${text ? `<p class="text-muted">${text}</p>` : ''}
    <code>${share}</code>
    <br><br><br>
    <br><br><br>
  `).join('')

  $('#create-wallet').classList.add('d-none')
  $('#wallet-backup').classList.remove('d-none')
  
})
