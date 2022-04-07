import https from 'https';

export async function getText(url){
  return new Promise(async (resolve, reject) => {
    https.get(url, (resp) => {
      let data = '';

      // A chunk of data has been received.
      resp.on('data', (chunk) => {
        data += chunk;
      });

      // The whole response has been received. Print out the result.
      resp.on('end', () => {
        resolve(data);
      });

    }).on("error", (err) => {
      reject(err);
    });
  })
}

export async function getJSON(url){
  return JSON.parse(await getText(url))
}
