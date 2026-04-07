import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import { Card, CardBody } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm } from '../../types/api'
import { TrendingUp, Layers, PieChart, CreditCard } from 'lucide-react'

interface RoutingCard {
  id: string
  title: string
  description: string
  icon: React.ElementType
  route: string
  algorithmType: string
  checkConfigured: () => boolean
}

export function RoutingHubPage() {
  const navigate = useNavigate()
  const { merchantId } = useMerchantStore()

  const { data: activeAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const { data: srConfig } = useSWR(
    merchantId ? [`/rule/get`, 'successRate', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, algorithm: 'successRate' })
  )

  const ROUTING_CARDS: RoutingCard[] = [
    {
      id: 'sr',
      title: 'Auth-Rate Based Routing',
      description: 'Dynamically route to the best-performing gateway based on real-time authorization rates.',
      icon: TrendingUp,
      route: '/routing/sr',
      algorithmType: 'successRate',
      checkConfigured: () => !!srConfig?.config?.data,
    },
    {
      id: 'rules',
      title: 'Rule-Based Routing',
      description: 'Declarative Euclid DSL rules to route payments based on conditions and attributes.',
      icon: Layers,
      route: '/routing/rules',
      algorithmType: 'advanced',
      checkConfigured: () => (activeAlgorithms || []).some(a => 
        (a.algorithm_data || a.algorithm)?.type === 'advanced'
      ),
    },
    {
      id: 'volume',
      title: 'Volume Split',
      description: 'Distribute payment traffic across gateways by configurable percentage splits.',
      icon: PieChart,
      route: '/routing/volume',
      algorithmType: 'volume_split',
      checkConfigured: () => (activeAlgorithms || []).some(a => 
        (a.algorithm_data || a.algorithm)?.type === 'volume_split'
      ),
    },
    {
      id: 'debit',
      title: 'Network Routing',
      description: 'Optimise debit network fees with acquirer-aware network-based routing.',
      icon: CreditCard,
      route: '/routing/debit',
      algorithmType: 'debitRouting',
      checkConfigured: () => false, // Not implemented yet
    },
  ]

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-gray-900">Routing Hub</h1>
        <p className="text-sm text-gray-500 mt-1">
          Click on any routing strategy to configure
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {ROUTING_CARDS.map((card) => {
          const Icon = card.icon
          const isConfigured = card.checkConfigured()
          return (
            <Card 
              key={card.id} 
              className="flex flex-col hover:border-brand-300 cursor-pointer transition-all hover:shadow-md"
              onClick={() => navigate(card.route)}
            >
              <CardBody className="flex-1 flex flex-col gap-3">
                <div className="flex items-start justify-between">
                  <div className="p-2 bg-brand-50 rounded-lg border border-[#1c2d50]">
                    <Icon size={20} className="text-brand-500" />
                  </div>
                  <Badge variant={isConfigured ? 'green' : 'gray'}>
                    {isConfigured ? 'Configured' : 'Not Configured'}
                  </Badge>
                </div>
                <div>
                  <h3 className="font-semibold text-gray-900">{card.title}</h3>
                  <p className="text-sm text-gray-500 mt-1">{card.description}</p>
                </div>
                <div className="mt-auto pt-2">
                  <span className="text-sm text-brand-600 font-medium">
                    {isConfigured ? 'Manage →' : 'Setup →'}
                  </span>
                </div>
              </CardBody>
            </Card>
          )
        })}
      </div>
    </div>
  )
}