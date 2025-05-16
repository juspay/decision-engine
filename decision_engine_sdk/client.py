import requests

class DecisionEngineClient:
    """
    Python SDK client for the Decision Engine APIs.

    This client provides convenient methods to interact with the following endpoints:
      - POST /decide-gateway
      - POST /update-gateway-score

    Example usage:
        from decision_engine_sdk.client import DecisionEngineClient
        client = DecisionEngineClient(base_url="http://localhost:8080")
        response = client.decide_gateway(payload)
        score_response = client.update_gateway_score(score_payload)

    Args:
        base_url (str): The base URL of the Decision Engine API server. Defaults to 'http://localhost:8080'.
    """
    def __init__(self, base_url: str = "http://localhost:8080"):
        """
        Initialize the DecisionEngineClient.

        Args:
            base_url (str): The base URL of the Decision Engine API server.
        """
        self.base_url = base_url.rstrip("/")

    def decide_gateway(self, payload: dict) -> dict:
        """
        Call the /decide-gateway endpoint to determine the optimal payment gateway.

        Args:
            payload (dict): The request body as per API spec. Must include merchantId, eligibleGatewayList, rankingAlgorithm, eliminationEnabled, and paymentInfo.

        Returns:
            dict: The API response containing gateway decision details.

        Raises:
            requests.HTTPError: If the server returns an HTTP error response.
        """
        url = f"{self.base_url}/decide-gateway"
        response = requests.post(url, json=payload)
        response.raise_for_status()
        return response.json()

    def update_gateway_score(self, payload: dict):
        """
        Call the /update-gateway-score endpoint to update the score/status for a gateway after a payment attempt.

        Args:
            payload (dict): The request body as per API spec. Must include merchantId, gateway, status, and paymentId.

        Returns:
            dict or str: The API response as dict if JSON, else raw text (e.g., 'Success').

        Raises:
            requests.HTTPError: If the server returns an HTTP error response.
        """
        url = f"{self.base_url}/update-gateway-score"
        response = requests.post(url, json=payload)
        response.raise_for_status()
        try:
            return response.json()
        except ValueError:
            return response.text 