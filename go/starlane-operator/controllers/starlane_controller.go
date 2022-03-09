/*
Copyright 2021.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package controllers

import (
	"context"
	"k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/api/resource"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
	"k8s.io/apimachinery/pkg/util/intstr"
	"time"

	"github.com/sethvargo/go-password/password"
	appsv1 "k8s.io/api/apps/v1"
	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"

	starlanev1alpha1 "github.com/mechtronium/starlane/api/v1alpha1"
)

// StarlaneReconciler reconciles a Starlane object
type StarlaneReconciler struct {
	client.Client
	Scheme *runtime.Scheme
}

//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlanes,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlanes/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=starlane.starlane.io,resources=starlanes/finalizers,verbs=update
//+kubebuilder:rbac:groups=apps,resources=deployments,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=core,resources=pods,verbs=get;list;
//+kubebuilder:rbac:groups=core,resources=services,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=core,resources=secrets,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=core,resources=persistentvolumeclaims,verbs=get;list;watch;create;update;patch;delete

// Reconcile is part of the main kubernetes reconciliation loop which aims to
// move the current state of the cluster closer to the desired state.
// TODO(user): Modify the Reconcile function to compare the state specified by
// the Starlane object against the actual cluster state, and then
// perform operations to make the cluster state reflect the state specified by
// the user.
//
// For more details, check Reconcile and its Result here:
// - https://pkg.go.dev/sigs.k8s.io/controller-runtime@v0.8.3/pkg/reconcile
func (r *StarlaneReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	log := log.FromContext(ctx)

	// Fetch the Starlane instance
	starlane := &starlanev1alpha1.Starlane{}
	err := r.Get(ctx, req.NamespacedName, starlane)
	if err != nil {
		if errors.IsNotFound(err) {
			// Request object not found, could have been deleted after reconcile request.
			// Owned objects are automatically garbage collected. For additional cleanup logic use finalizers.
			// Return and don't requeue
			log.Info("Starlane resource not found. Ignoring since object must be deleted")
			return ctrl.Result{}, nil
		}
		// Error reading the object - requeue the request.
		log.Error(err, "Failed to get Starlane")
		return ctrl.Result{}, err
	}

	// postgres4Keycloak
	{
		pvc := &corev1.PersistentVolumeClaim{}
		err = r.Get(ctx, types.NamespacedName{Name: postgres4KeycloakName(starlane), Namespace: starlane.Namespace}, pvc)
		if err != nil && errors.IsNotFound(err) {
			// Define a new deployment
			dep := r.postgres4KeycloakPvc(starlane)
			log.Info("Creating a new Pvc", "Pvc.Namespace", dep.Namespace, "Pvc.Name", dep.Name)
			err = r.Create(ctx, dep)
			if err != nil {
				log.Error(err, "Failed to create new Postgres PVC", "Pvc.Namespace", dep.Namespace, "Pvc.Name", dep.Name)
				return ctrl.Result{}, err
			}
			// Pvc created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Pvc")
			return ctrl.Result{}, err
		}

		secret := &corev1.Secret{}
		err = r.Get(ctx, types.NamespacedName{Name: starlane.Name, Namespace: starlane.Namespace}, secret)
		if err != nil && errors.IsNotFound(err) {
			// Define a new deployment
			dep, gen_err := r.generateSecret(starlane)
			if gen_err != nil {
				log.Error(gen_err, "Failed to create a password", "Namespace", dep.Namespace, "Name", dep.Name)
				return ctrl.Result{}, gen_err
			}
			log.Info("Creating a new Secret", "Namespace", dep.Namespace, "Name", dep.Name)
			err = r.Create(ctx, dep)
			if err != nil {
				log.Error(err, "Failed to create new Postgres Secret", "Namespace", dep.Namespace, "Name", dep.Name)
				return ctrl.Result{}, err
			}
			// Pvc created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Secret")
			return ctrl.Result{}, err
		}

		postgres := &appsv1.Deployment{}
		err = r.Get(ctx, types.NamespacedName{Name: postgres4KeycloakName(starlane), Namespace: starlane.Namespace}, postgres)
		if err != nil && errors.IsNotFound(err) {
			dep := r.postgres4KeycloakDeployment(starlane)
			log.Info("Creating a new Postgres deployment", "Deployment.Namespace", dep.Namespace, "Deployment.Name", dep.Name)
			err = r.Create(ctx, dep)
			if err != nil {
				log.Error(err, "Failed to create new Postgres", "Deployment.Namespace", dep.Namespace, "Deployment.Name", dep.Name)
				return ctrl.Result{}, err
			}
			// Pvc created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Secret")
			return ctrl.Result{}, err
		}

		service := &corev1.Service{}
		err = r.Get(ctx, types.NamespacedName{Name: postgres4KeycloakName(starlane), Namespace: starlane.Namespace}, service)
		if err != nil && errors.IsNotFound(err) {
			dep := r.postgres4KeycloakService(starlane)
			log.Info("Creating a new Postgres service", "Namespace", dep.Namespace, "Name", dep.Name)
			err = r.Create(ctx, dep)
			if err != nil {
				log.Error(err, "Failed to create new Postgres Service", "Namespace", dep.Namespace, "Name", dep.Name)
				return ctrl.Result{}, err
			}
			// Pvc created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Service")
			return ctrl.Result{}, err
		}
	}

	{
		keycloak := &appsv1.Deployment{}
		err = r.Get(ctx, types.NamespacedName{Name: keycloakName(starlane), Namespace: starlane.Namespace}, keycloak)
		if err != nil && errors.IsNotFound(err) {
			dep := r.keycloakDeployment(starlane)
			log.Info("Creating a new Keycloak deployment", "Namespace", dep.Namespace, "Name", dep.Name)
			err = r.Create(ctx, dep)
			if err != nil {
				log.Error(err, "Failed to create new Keycloak", "Namespace", dep.Namespace, "Name", dep.Name)
				return ctrl.Result{}, err
			}
			// Pvc created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Keycloak")
			return ctrl.Result{}, err
		}

		service := &corev1.Service{}
		err = r.Get(ctx, types.NamespacedName{Name: keycloakName(starlane), Namespace: starlane.Namespace}, service)
		if err != nil && errors.IsNotFound(err) {
			dep := r.keycloakService(starlane)
			log.Info("Creating a new Keycloak service", "Namespace", dep.Namespace, "Name", dep.Name)
			err = r.Create(ctx, dep)
			if err != nil {
				log.Error(err, "Failed to create new Keycloak Service", "Namespace", dep.Namespace, "Name", dep.Name)
				return ctrl.Result{}, err
			}
			// Pvc created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Keycloak")
			return ctrl.Result{}, err
		}
	}

	// Check if the deployment already exists, if not create a new one
	{
		found := &appsv1.Deployment{}
		err = r.Get(ctx, types.NamespacedName{Name: starlane.Name, Namespace: starlane.Namespace}, found)
		if err != nil && errors.IsNotFound(err) {
			// Define a new deployment
			dep := r.deploymentForStarlane(starlane)
			log.Info("Creating a new Deployment", "Deployment.Namespace", dep.Namespace, "Deployment.Name", dep.Name)
			err = r.Create(ctx, dep)
			if err != nil {
				log.Error(err, "Failed to create new Deployment", "Deployment.Namespace", dep.Namespace, "Deployment.Name", dep.Name)
				return ctrl.Result{}, err
			}
			// Deployment created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Deployment")
			return ctrl.Result{}, err
		}
	}

	{
		// Check if the web service already exists, if not create a new one
		service := &corev1.Service{}
		err = r.Get(ctx, types.NamespacedName{Name: starlane.Name + "-web", Namespace: starlane.Namespace}, service)
		if err != nil && errors.IsNotFound(err) {
			// Define a new deployment
			srv := r.webServiceForStarlane(starlane)
			log.Info("Creating a new Service", "Service.Namespace", srv.Namespace, "Service.Name", srv.Name)
			err = r.Create(ctx, srv)
			if err != nil {
				log.Error(err, "Failed to create new Service", "Service.Namespace", srv.Namespace, "Service.Name", srv.Name)
				return ctrl.Result{}, err
			}
			// Deployment created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Service")
			return ctrl.Result{}, err
		} else if service.Spec.Type != starlane.Spec.WebServiceType {
			service.Spec.Type = starlane.Spec.WebServiceType
			err = r.Update(ctx, service)
			if err != nil {
				log.Error(err, "Failed to update Service", "Service.Namespace", service.Namespace, "Service.Name", service.Name)
				return ctrl.Result{}, err
			}
			// Ask to requeue after 1 minute in order to give enough time for the
			// pods be created on the cluster side and the operand be able
			// to do the next update step accurately.
			return ctrl.Result{RequeueAfter: time.Minute}, nil
		}
	}

	{
		// Check if the web service already exists, if not create a new one
		service := &corev1.Service{}
		err = r.Get(ctx, types.NamespacedName{Name: starlane.Name + "-gateway", Namespace: starlane.Namespace}, service)
		if err != nil && errors.IsNotFound(err) {
			// Define a new deployment
			srv := r.gatewayServiceForStarlane(starlane)
			log.Info("Creating a new Service", "Service.Namespace", srv.Namespace, "Service.Name", srv.Name)
			err = r.Create(ctx, srv)
			if err != nil {
				log.Error(err, "Failed to create new Service", "Service.Namespace", srv.Namespace, "Service.Name", srv.Name)
				return ctrl.Result{}, err
			}
			// Deployment created successfully - return and requeue
			return ctrl.Result{Requeue: true}, nil
		} else if err != nil {
			log.Error(err, "Failed to get Service")
			return ctrl.Result{}, err
		} else if service.Spec.Type != starlane.Spec.GatewayServiceType {
			service.Spec.Type = starlane.Spec.GatewayServiceType
			err = r.Update(ctx, service)
			if err != nil {
				log.Error(err, "Failed to update Service", "Service.Namespace", service.Namespace, "Service.Name", service.Name)
				return ctrl.Result{}, err
			}
			// Ask to requeue after 1 minute in order to give enough time for the
			// pods be created on the cluster side and the operand be able
			// to do the next update step accurately.
			return ctrl.Result{RequeueAfter: time.Minute}, nil
		}
	}

	// your logic here

	return ctrl.Result{}, nil
}

// deploymentForStarlane returns a memcached Deployment object
func (r *StarlaneReconciler) deploymentForStarlane(m *starlanev1alpha1.Starlane) *appsv1.Deployment {
	ls := labelsForStandalone(m.Name)
	replicas := int32(1)

	dep := &appsv1.Deployment{
		ObjectMeta: metav1.ObjectMeta{
			Name:      m.Name,
			Namespace: m.Namespace,
		},
		Spec: appsv1.DeploymentSpec{
			Replicas: &replicas,
			Selector: &metav1.LabelSelector{
				MatchLabels: ls,
			},
			Template: corev1.PodTemplateSpec{
				ObjectMeta: metav1.ObjectMeta{
					Labels: ls,
				},
				Spec: corev1.PodSpec{
					Containers: []corev1.Container{{
						Image: "starlane/starlane:latest",
						Name:  "starlane",
						Args:  []string{"serve", "--with-external"},
						Env: []corev1.EnvVar{
							{Name: "STARLANE_KUBERNETES_INSTANCE_NAME", Value: m.Name},
							{Name: "STARLANE_KEYCLOAK_URL", Value: keycloakName(m)+":8080"},
							{Name: "NAMESPACE", Value: m.Namespace},
							{
								Name: "STARLANE_PASSWORD",
								ValueFrom: &corev1.EnvVarSource{
									SecretKeyRef: &corev1.SecretKeySelector{
										LocalObjectReference: corev1.LocalObjectReference{
											Name: m.Name,
										},
										Key: "password",
									},
								},
							},
						},
						Ports: []corev1.ContainerPort{{
							ContainerPort: 4343,
							Name:          "gateway",
						}, {
							ContainerPort: 8080,
							Name:          "http",
						}},
					}},
				},
			},
		},
	}
	// Set Starlane instance as the owner and controller
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

// deploymentForStarlane returns a memcached Deployment object
func (r *StarlaneReconciler) webServiceForStarlane(m *starlanev1alpha1.Starlane) *corev1.Service {

	dep := &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:      m.Name + "-web",
			Namespace: m.Namespace,
		},
		Spec: corev1.ServiceSpec{
			Type: m.Spec.WebServiceType,
			Ports: []corev1.ServicePort{
				{Name: "http",
					Port:       80,
					TargetPort: intstr.FromInt(8080),
					Protocol:   corev1.ProtocolTCP,
				},
			},
			Selector: labelsForWeb(m.Name),
		},
	}
	// Set Starlane instance as the owner and controller
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

// deploymentForStarlane returns a memcached Deployment object
func (r *StarlaneReconciler) gatewayServiceForStarlane(m *starlanev1alpha1.Starlane) *corev1.Service {

	dep := &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:      m.Name + "-gateway",
			Namespace: m.Namespace,
		},
		Spec: corev1.ServiceSpec{
			Type: m.Spec.GatewayServiceType,
			Ports: []corev1.ServicePort{
				{Name: "gateway",
					Port:       4343,
					TargetPort: intstr.FromInt(4343),
					Protocol:   corev1.ProtocolTCP,
				},
			},
			Selector: labelsForGateway(m.Name),
		},
	}
	// Set Starlane instance as the owner and controller
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

func postgres4KeycloakName(m *starlanev1alpha1.Starlane) string {
	return m.Name + "-postgres-4-keycloak"
}

func keycloakName(m *starlanev1alpha1.Starlane) string {
	return m.Name + "-keycloak"
}

func labelsForStandalone(galaxy string) map[string]string {
	return map[string]string{"app": "starlane", "galaxy": galaxy, "web": "true", "gateway": "true"}
}

func labelsForWeb(galaxy string) map[string]string {
	return map[string]string{"app": "starlane", "galaxy": galaxy, "web": "true"}
}

func labelsForGateway(galaxy string) map[string]string {
	return map[string]string{"app": "starlane", "galaxy": galaxy, "gateway": "true"}
}

// getPodNames returns the pod names of the array of pods passed in
func getPodNames(pods []corev1.Pod) []string {
	var podNames []string
	for _, pod := range pods {
		podNames = append(podNames, pod.Name)
	}
	return podNames
}

func (r *StarlaneReconciler) generateSecret(m *starlanev1alpha1.Starlane) (*corev1.Secret, error) {
	password, err := password.Generate(16, 4, 4, false, false)

	if err != nil {
		return &corev1.Secret{}, err
	} else {
		data := map[string]string{"password": password}
		dep := &corev1.Secret{
			ObjectMeta: metav1.ObjectMeta{
				Name:      m.Name,
				Namespace: m.Namespace,
			},
			StringData: data,
		}
		ctrl.SetControllerReference(m, dep, r.Scheme)
		return dep, nil
	}
}

func (r *StarlaneReconciler) postgres4KeycloakPvc(m *starlanev1alpha1.Starlane) *corev1.PersistentVolumeClaim {
	name := postgres4KeycloakName(m)
	dep := &corev1.PersistentVolumeClaim{
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: m.Namespace,
		},
		Spec: corev1.PersistentVolumeClaimSpec{StorageClassName: &m.Spec.StorageClass,
			AccessModes: []corev1.PersistentVolumeAccessMode{"ReadWriteOnce"},
			Resources: corev1.ResourceRequirements{
				Requests: corev1.ResourceList{
					corev1.ResourceStorage: resource.MustParse("5Gi"),
				},
			}},
	}
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

// deploymentForStarlane returns a memcached Deployment object
func (r *StarlaneReconciler) postgres4KeycloakDeployment(m *starlanev1alpha1.Starlane) *appsv1.Deployment {

	name := postgres4KeycloakName(m)

	ls := map[string]string{"name": name}
	replicas := int32(1)

	dep := &appsv1.Deployment{
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: m.Namespace,
		},
		Spec: appsv1.DeploymentSpec{
			Replicas: &replicas,
			Selector: &metav1.LabelSelector{
				MatchLabels: ls,
			},
			Template: corev1.PodTemplateSpec{
				ObjectMeta: metav1.ObjectMeta{
					Labels: ls,
				},
				Spec: corev1.PodSpec{
					Containers: []corev1.Container{{
						Image: "postgres:14.2-alpine",
						Name:  "postgres",
						Args:  []string{},
						Env: []corev1.EnvVar{
							{
								Name:  "PGDATA",
								Value: "/var/lib/postgresql/data",
							},
							{
								Name: "POSTGRES_PASSWORD",
								ValueFrom: &corev1.EnvVarSource{
									SecretKeyRef: &corev1.SecretKeySelector{
										LocalObjectReference: corev1.LocalObjectReference{
											Name: m.Name,
										},
										Key: "password",
									},
								},
							},
						},
						Ports: []corev1.ContainerPort{{
							ContainerPort: 5432,
							Name:          "postgres",
						}},
						VolumeMounts: []corev1.VolumeMount{
							{
								Name:      "data",
								MountPath: "/var/lib/postgresql/data",
								ReadOnly:  false,
							},
						},
					}},
					Volumes: []corev1.Volume{
						{
							Name:         "data",
							VolumeSource: corev1.VolumeSource{PersistentVolumeClaim: &corev1.PersistentVolumeClaimVolumeSource{ClaimName: name}},
						},
					},
				},
			},
		},
	}
	// Set Starlane instance as the owner and controller
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

// deploymentForStarlane returns a memcached Deployment object
func (r *StarlaneReconciler) postgres4KeycloakService(m *starlanev1alpha1.Starlane) *corev1.Service {

	name := postgres4KeycloakName(m)

	dep := &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: m.Namespace,
		},
		Spec: corev1.ServiceSpec{
			Type: corev1.ServiceTypeClusterIP,
			Ports: []corev1.ServicePort{
				{Name: "postgres",
					Port:       5432,
					TargetPort: intstr.FromInt(5432),
					Protocol:   corev1.ProtocolTCP,
				},
			},
			Selector: map[string]string{"name": name},
		},
	}
	// Set Starlane instance as the owner and controller
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

func (r *StarlaneReconciler) keycloakDeployment(m *starlanev1alpha1.Starlane) *appsv1.Deployment {

	name := keycloakName(m)
	postgres := postgres4KeycloakName(m)

	ls := map[string]string{"name": name}
	replicas := int32(1)

	dep := &appsv1.Deployment{
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: m.Namespace,
		},
		Spec: appsv1.DeploymentSpec{
			Replicas: &replicas,
			Selector: &metav1.LabelSelector{
				MatchLabels: ls,
			},
			Template: corev1.PodTemplateSpec{
				ObjectMeta: metav1.ObjectMeta{
					Labels: ls,
				},
				Spec: corev1.PodSpec{
					Containers: []corev1.Container{{
						Image: "jboss/keycloak:13.0.1",
						Name:  "keycloak",
						Args:  []string{},
						Env: []corev1.EnvVar{
							{
								Name:  "DB_VENDOR",
								Value: "postgres",
							},
							{
								Name:  "DB_ADDR",
								Value: postgres,
							},
							{
								Name:  "DB_PORT",
								Value: "5432",
							},
							{
								Name:  "DB_USER",
								Value: "postgres",
							},
							{
								Name:  "DB_DATABASE",
								Value: "postgres",
							},
							{
								Name:  "KEYCLOAK_USER",
								Value: "hyperuser",
							},
							{
								Name:  "KEYCLOAK_CORS",
								Value: "true",
							},
							{
								Name:  "KEYCLOAK_ALWAYS_HTTPS",
								Value: "false",
							},
							{
								Name:  "PROTOCOL",
								Value: "http",
							},
							{
								Name:  "PROXY_ADDRESS_FORWARDING",
								Value: "true",
							},
							{
								Name: "KEYCLOAK_PASSWORD",
								ValueFrom: &corev1.EnvVarSource{
									SecretKeyRef: &corev1.SecretKeySelector{
										LocalObjectReference: corev1.LocalObjectReference{
											Name: m.Name,
										},
										Key: "password",
									},
								},
							},
							{
								Name: "DB_PASSWORD",
								ValueFrom: &corev1.EnvVarSource{
									SecretKeyRef: &corev1.SecretKeySelector{
										LocalObjectReference: corev1.LocalObjectReference{
											Name: m.Name,
										},
										Key: "password",
									},
								},
							},
						},
						Ports: []corev1.ContainerPort{{
							ContainerPort: 8080,
							Name:          "keycloak",
						}},
					}},
				},
			},
		},
	}
	// Set Starlane instance as the owner and controller
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

// deploymentForStarlane returns a memcached Deployment object
func (r *StarlaneReconciler) keycloakService(m *starlanev1alpha1.Starlane) *corev1.Service {

	name := keycloakName(m)

	dep := &corev1.Service{
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: m.Namespace,
		},
		Spec: corev1.ServiceSpec{
			Type: corev1.ServiceTypeLoadBalancer,
			Ports: []corev1.ServicePort{
				{Name: "keycloak",
					Port:       8080,
					TargetPort: intstr.FromInt(8080),
					Protocol:   corev1.ProtocolTCP,
				},
			},
			Selector: map[string]string{"name": name},
		},
	}
	// Set Starlane instance as the owner and controller
	ctrl.SetControllerReference(m, dep, r.Scheme)
	return dep
}

// SetupWithManager sets up the controller with the Manager.
func (r *StarlaneReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&starlanev1alpha1.Starlane{}).
		Owns(&appsv1.Deployment{}).
		Owns(&corev1.Service{}).
		Owns(&corev1.Secret{}).
		Owns(&corev1.PersistentVolumeClaim{}).
		Complete(r)
}
