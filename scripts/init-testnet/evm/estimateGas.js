import {ethers} from 'ethers';

async function main() {
    // Define the Ethereum RPC URL
    const ethereumRpcUrl = "https://ws.nice.hydration.cloud"; // Replace with your Ethereum RPC URL
    //const ethereumRpcUrl = "https://rpc.hydradx.cloud"; // Replace with your Ethereum RPC URL

    //const ethereumRpcUrl = "http://127.0.0.1:9988"; // Replace with your Ethereum RPC URL

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

        try {
            const dispatchContractAddress = '0x0000000000000000000000000000000000000401';
            //const extrinsicData = '0x3b05000000000500000000a0724e18090000000000000000000000000000000000000000000000000000'; // Your extrinsic data
           // const extrinsicData = '0xcb0014000000'; // Your extrinsic data
            const extrinsicData = '0x4f003679d1d8e31d312a55f7ca994773b6a4fc7a92f07d898ae86bad4f3cab303c49000000000b00a0724e1809'; // Your extrinsic data

            const estimateGasParams = {
                from: wallet.address, // The address initiating the transaction
                to: dispatchContractAddress,
                data: extrinsicData,
                value: 0 // If you're not sending any Ether with the transaction
            };

            const estimatedGas = await provider.estimateGas(estimateGasParams);

            console.log("Estimated Gas:", estimatedGas.toString());

        } catch (error) {
            console.error("Error estimating gas:", error);
        }



    } catch (error) {
        console.error("Error:", error);
    }
}

main();