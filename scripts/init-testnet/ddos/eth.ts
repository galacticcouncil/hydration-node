const { ethers } = require('ethers');

async function main() {
    // Define the Ethereum RPC URL
    //const ethereumRpcUrl = "https://rpc.nice.hydration.cloud"; // Replace with your Ethereum RPC URL

    const ethereumRpcUrl = "http://127.0.0.1:9988"; // Replace with your Ethereum RPC URL

    // Create a provider using the Ethereum RPC URL
    const provider = new ethers.JsonRpcProvider(ethereumRpcUrl);

    try {
        // Example: Fetch the Ethereum block number
        const blockNumber = await provider.getBlockNumber();
        console.log("Ethereum Block Number:", blockNumber);

        // Example: Fetch the balance of an Ethereum address
        const address = "0x222222ff7Be76052e023Ec1a306fCca8F9659D80"; // Replace with the Ethereum address you want to check
        const balance = await provider.getBalance(address);
        console.log("Ethereum Balance (Wei):", balance.toString());


        const privateKey = '42d8d953e4f9246093a33e9ca6daa078501012f784adfe4bbed57918ff13be14';
        const wallet = new ethers.Wallet(privateKey, provider);

        const dispatchContractAddress = '0x0000000000000000000000000000000000000401';

        let nonce = await provider.getTransactionCount(wallet.address, 'latest');


        for (let index = 0; index < 20000; index++) {
            try {
                const extrinsicData = '0x3b05000000000500000000a0724e18090000000000000000000000000000000000000000000000000000'; // Replace with your extrinsic data
                console.log("firing evm tx");

                const tx = {
                    to: dispatchContractAddress,
                    value: 0,
                    nonce: nonce + index,
                    gasLimit: 100000, // Adjust the gas limit accordingly
                    gasPrice: ethers.parseUnits('8000000' + index, 'wei'), // Adjust the gas price accordingly
                    data: extrinsicData,
                };

                const signer2= wallet.connect(provider); // Connect the wallet to the provider

                await signer2.sendTransaction(tx);

            } catch (error) {
                console.error('Error:', error);
            }
        }





    } catch (error) {
        console.error("Error:", error);
    }
}

main();